//! Port of the non-VAD silence DSP from `stable_whisper.stabilization.nonvad`.
//!
//! This is the deterministic signal-processing core of stable-ts's
//! `suppress_silence` (with `vad=False`, the submate default): no ML model, just
//! `audio2loudness` (a per-token loudness envelope) feeding `wav2mask` (a
//! quantize/pool/timing roundtrip that yields a per-token silence mask). The
//! mask marks which whisper timestamp tokens fall on audible audio.
//!
//! ## Exact-float parity
//!
//! Upstream runs on PyTorch tensors in `f32`, so the *order* and *width* of the
//! arithmetic are load-bearing — matching torch within `1e-6` requires
//! reproducing two non-obvious details:
//!
//! * **`F.interpolate(mode="linear", align_corners=False)`** computes the
//!   source coordinate in `f32`: `src = f32(scale * (i + 0.5) - 0.5)` where
//!   `scale = f32(input_len) / f32(output_len)`, then point-samples the two
//!   neighbours and lerps. Doing the coordinate math in `f64` drifts by ~7e-4
//!   at high indices because the `f32` scale loses mantissa bits; we deliberately
//!   keep it in `f32`.
//! * **`avg_pool1d` with reflect padding** (`k=5`, `p=2`) mirrors the signal at
//!   each edge *without* repeating the boundary sample: `[x2, x1, x0..xn, xn-2,
//!   xn-3]`.
//!
//! The `mask2timing`/`timing2mask` roundtrip upstream goes token-index ->
//! seconds (`/ TOKENS_PER_SECOND`) -> token-index (`round(* TOKENS_PER_SECOND)`),
//! which is the identity on integer indices; the only thing it actually does is
//! drop silence runs of `(end - start) <= 0.1 s`, i.e. `<= 5` tokens. We keep
//! the comparison in token units (`run_len * 1.0 / 50.0 > 0.1`) so the threshold
//! rounds the same way.

/// `whisper.audio.N_SAMPLES_PER_TOKEN` (`HOP_LENGTH * 2`): one timestamp token
/// spans this many 16 kHz samples.
pub const N_SAMPLES_PER_TOKEN: usize = 320;

/// `whisper.audio.TOKENS_PER_SECOND` (`SAMPLE_RATE / N_SAMPLES_PER_TOKEN`).
///
/// `f64` because upstream converts token indices to *seconds* with it (numpy
/// `float64`) and then thresholds the duration; the comparison is sensitive to
/// that width (see [`wav2mask`]).
pub const TOKENS_PER_SECOND: f64 = 50.0;

/// Per-token loudness envelope, mirroring `nonvad.audio2loudness` with the
/// default `samples_per_unit = N_SAMPLES_PER_TOKEN`.
///
/// Returns `None` for clips too short to produce more than two tokens (upstream
/// falls through to an implicit `None`), matching the `token_count > 2` guard.
/// A near-silent clip (`threshold < 1e-5`) yields an all-zero envelope.
pub fn audio2loudness(audio: &[f32]) -> Option<Vec<f32>> {
    let n = audio.len();
    let abs: Vec<f32> = audio.iter().map(|x| x.abs()).collect();

    // threshold = the k-th largest |sample|, k = int(n * 0.001).
    // (torch.topk(k)[-1]); for k == 0 upstream uses quantile(0.999) instead.
    let k = ((n as f64) * 0.001) as usize;
    let threshold = if k != 0 {
        let mut sorted = abs.clone();
        // Descending; kth largest is index k-1. NaN-free audio, so total order.
        sorted.sort_by(|a, b| b.partial_cmp(a).unwrap());
        sorted[k - 1]
    } else {
        quantile_999(&abs)
    };

    let token_count = ((n as f64) / (N_SAMPLES_PER_TOKEN as f64)).round() as usize + 1;
    if token_count <= 2 {
        return None;
    }
    if threshold < 1e-5 {
        return Some(vec![0.0; token_count]);
    }

    // audio / min(1.0, threshold * 1.75)
    let divisor = (threshold * 1.75).min(1.0);
    let scaled: Vec<f32> = abs.iter().map(|x| x / divisor).collect();

    Some(interpolate_linear(&scaled, token_count))
}

/// `numpy.quantile(x, 0.999)` with linear interpolation, used only when
/// `k == 0` (clips under 1000 samples). Kept in `f32` to match upstream.
fn quantile_999(x: &[f32]) -> f32 {
    if x.is_empty() {
        return 0.0;
    }
    let mut sorted = x.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let pos = 0.999_f32 * (sorted.len() as f32 - 1.0);
    let lo = pos.floor() as usize;
    let hi = (lo + 1).min(sorted.len() - 1);
    let frac = pos - lo as f32;
    sorted[lo] * (1.0 - frac) + sorted[hi] * frac
}

/// `F.interpolate(x, size=out_len, mode="linear", align_corners=False)`.
///
/// Point-samples the source signal at `src = f32(scale * (i + 0.5) - 0.5)` and
/// lerps between the two surrounding samples; `src` clamps to `0` on the left
/// and reuses the last sample on the right. The coordinate math stays in `f32`
/// on purpose (see module docs).
fn interpolate_linear(x: &[f32], out_len: usize) -> Vec<f32> {
    let in_len = x.len();
    let scale = (in_len as f32) / (out_len as f32);
    (0..out_len)
        .map(|i| {
            let src = scale * (i as f32 + 0.5) - 0.5;
            let src = if src < 0.0 { 0.0 } else { src };
            let i0 = src.floor() as usize;
            let w = src - i0 as f32;
            let i1 = if i0 + 1 < in_len { i0 + 1 } else { i0 };
            x[i0] * (1.0 - w) + x[i1] * w
        })
        .collect()
}

/// Per-token silence-suppression mask, mirroring `nonvad.wav2mask` for already
/// 16 kHz mono audio (`q_levels = 20`, `k_size = 5`).
///
/// Returns a boolean vector aligned to the loudness/token grid where `true`
/// marks tokens to *keep* (audible). The two `None`-like upstream returns are
/// distinguished:
///
/// * an all-`true` mask (the audio is *entirely silent*: upstream returns the
///   inverted empty quantized mask), and
/// * `None` when there is no silence to suppress at all.
pub fn wav2mask(audio: &[f32]) -> Option<Vec<bool>> {
    let loudness = audio2loudness(audio)?;
    let len = loudness.len();

    // avg_pool1d(reflect-pad, k=5, stride=1) when the padding fits.
    let k_size = 5usize;
    let p = k_size / 2;
    let pooled = if p > 0 && p < len {
        avg_pool1d_reflect(&loudness, k_size)
    } else {
        loudness.clone()
    };

    // quantize: mask.mul(20).round().bool()  (0 -> false, nonzero -> true)
    let q_levels = 20.0_f32;
    let quantized: Vec<bool> = pooled.iter().map(|v| (v * q_levels).round() != 0.0).collect();

    if !quantized.iter().any(|&b| b) {
        // entirely silent: upstream returns ~mask, i.e. all-true.
        return Some(vec![true; len]);
    }

    // mask2timing -> drop runs of (end - start) <= 0.1 s -> timing2mask -> invert.
    //
    // The duration filter is upstream's `(e - s) > 0.1` over *seconds* in numpy
    // `float64`: token indices become seconds via `/ TOKENS_PER_SECOND` first,
    // then the difference is thresholded. A run spanning exactly 5 tokens
    // (`4.08 - 3.98`) is kept because that `f64` subtraction is `0.1 + ~5e-17`,
    // strictly greater than `0.1`. Doing this in token units (`5.0 / 50.0`) or
    // in `f32` would instead drop it.
    let runs = mask_runs(&quantized);
    let kept: Vec<(usize, usize)> = runs
        .into_iter()
        .filter(|&(s, e)| {
            let s_sec = s as f64 / TOKENS_PER_SECOND;
            let e_sec = e as f64 / TOKENS_PER_SECOND;
            (e_sec - s_sec) > 0.1
        })
        .collect();

    let mut silence = vec![false; len];
    for (s, e) in kept {
        // timing2mask fills [start, end] inclusive on the round-tripped indices.
        let end = e.min(len.saturating_sub(1));
        for slot in silence.iter_mut().take(end + 1).skip(s) {
            *slot = true;
        }
    }
    let mask: Vec<bool> = silence.iter().map(|&s| !s).collect();

    if !mask.iter().any(|&b| b) {
        // no silence after filtering: upstream returns None.
        return None;
    }
    Some(mask)
}

/// `torch.avg_pool1d(F.pad(x, (p, p), "reflect"), kernel_size=k, stride=1)`.
///
/// Reflect padding mirrors the interior without repeating the edge sample.
fn avg_pool1d_reflect(x: &[f32], k_size: usize) -> Vec<f32> {
    let p = k_size / 2;
    let n = x.len();
    let mut padded = Vec::with_capacity(n + 2 * p);
    for j in (1..=p).rev() {
        padded.push(x[j]);
    }
    padded.extend_from_slice(x);
    for j in 1..=p {
        padded.push(x[n - 1 - j]);
    }
    let inv_k = 1.0 / k_size as f32;
    (0..n)
        .map(|i| {
            let sum: f32 = padded[i..i + k_size].iter().sum();
            sum * inv_k
        })
        .collect()
}

/// `mask2timing`: contiguous `true` runs as half-open `[start, end)` token
/// indices (the upstream `silent_ends` is the index just past the run).
fn mask_runs(mask: &[bool]) -> Vec<(usize, usize)> {
    let mut runs = Vec::new();
    let mut start: Option<usize> = None;
    for (i, &b) in mask.iter().enumerate() {
        match (b, start) {
            (true, None) => start = Some(i),
            (false, Some(s)) => {
                runs.push((s, i));
                start = None;
            }
            _ => {}
        }
    }
    if let Some(s) = start {
        runs.push((s, mask.len()));
    }
    runs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interpolate_linear_matches_torch_reference() {
        // F.interpolate([0..10], size=4, linear, align_corners=False)
        let x: Vec<f32> = (0..10).map(|v| v as f32).collect();
        let out = interpolate_linear(&x, 4);
        let expected = [0.75_f32, 3.25, 5.75, 8.25];
        for (a, e) in out.iter().zip(expected) {
            assert!((a - e).abs() < 1e-6, "{a} != {e}");
        }
    }

    #[test]
    fn avg_pool_reflect_matches_torch_reference() {
        let x = [1.0_f32, 2.0, 3.0, 4.0, 5.0];
        let out = avg_pool1d_reflect(&x, 5);
        let expected = [2.2_f32, 2.4, 3.0, 3.6, 3.8];
        for (a, e) in out.iter().zip(expected) {
            assert!((a - e).abs() < 1e-6, "{a} != {e}");
        }
    }

    #[test]
    fn short_clip_yields_no_loudness() {
        // token_count = round(320/320)+1 = 2, not > 2.
        assert!(audio2loudness(&vec![0.5; 320]).is_none());
    }

    #[test]
    fn near_silent_clip_is_all_zero_loudness() {
        let loud = audio2loudness(&vec![1e-7; 4000]).expect("token_count > 2");
        assert!(loud.iter().all(|&v| v == 0.0));
    }

    #[test]
    fn entirely_silent_audio_keeps_all_tokens() {
        // All-zero audio -> all-zero loudness -> empty quantized mask -> all-true.
        let mask = wav2mask(&vec![0.0; 4000]).expect("entirely silent returns ~mask");
        assert!(mask.iter().all(|&b| b));
    }

    #[test]
    fn mask_runs_finds_contiguous_true_spans() {
        let m = [false, true, true, false, true];
        assert_eq!(mask_runs(&m), vec![(1, 3), (4, 5)]);
    }
}
