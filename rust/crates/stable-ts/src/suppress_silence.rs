//! Port of the non-VAD silence DSP from `stable_whisper.stabilization.nonvad`
//! (the `vad=False` default path): [`audio2loudness`] and [`wav2mask`].
//!
//! This is the signal-processing core of timestamp stabilization. No ML model
//! is involved on this path, so the math is deterministic and reproduces the
//! Python/torch output exactly within a tight `f32` tolerance.
//!
//! The upstream pipeline is:
//!
//! 1. **[`audio2loudness`]** — take `|audio|`, derive a 0.1%-loudest threshold
//!    (`topk`), normalize by `min(1, threshold * 1.75)`, then linearly
//!    interpolate (torch `F.interpolate(mode="linear", align_corners=False)`)
//!    down to `round(n / N_SAMPLES_PER_TOKEN) + 1` per-token loudness values.
//! 2. **[`wav2mask`]** — smooth the loudness with an averaging pool
//!    (`avg_pool1d`, kernel 5, reflect-padded), quantize (`round(x * q_levels)`),
//!    and treat any non-zero bucket as "loud". Contiguous loud runs become
//!    silence-vs-speech timings; silence shorter than 0.1 s is dropped, then the
//!    surviving silence is inverted into the returned suppression mask.
//!
//! Constants mirror `whisper_compatibility`: `SAMPLE_RATE = 16000`,
//! `N_SAMPLES_PER_TOKEN = 320`, `TOKENS_PER_SECOND = 50`. The audio fed in is
//! already 16 kHz mono `f32`, so upstream's `audio_to_tensor_resample` is a
//! no-op here and is intentionally not ported.

/// `HOP_LENGTH * 2` — samples represented by one decoder token.
const N_SAMPLES_PER_TOKEN: usize = 320;

/// `SAMPLE_RATE / N_SAMPLES_PER_TOKEN` — loudness/token units per second.
const TOKENS_PER_SECOND: f32 = 50.0;

/// Per-token loudness envelope from a 16 kHz mono waveform.
///
/// Mirrors `audio2loudness(audio_tensor, samples_per_unit=None)`:
///
/// * `audio = audio.abs()`
/// * `k = int(n * 0.001)`; `threshold = topk(audio, k)[-1]` (the `k`-th largest
///   magnitude). `k` is 0 only for fewer than 1000 samples, which never happens
///   on real audio; the upstream `quantile(0.999)` fallback for that case is not
///   needed here, so a tiny clip yields an empty envelope (`None` -> `[]`).
/// * `token_count = round(n / N_SAMPLES_PER_TOKEN) + 1`. Upstream returns `None`
///   (here: an empty `Vec`) when `token_count <= 2`, and an all-zero envelope
///   when `threshold < 1e-5`.
/// * normalize by `min(1.0, threshold * 1.75)`, then linearly interpolate to
///   `token_count` samples (`F.interpolate(mode="linear", align_corners=False)`).
///
/// All arithmetic is `f32` to match the torch tensor dtype on this path.
#[must_use]
pub fn audio2loudness(audio: &[f32]) -> Vec<f32> {
    let n = audio.len();
    let mut mag: Vec<f32> = audio.iter().map(|x| x.abs()).collect();

    let token_count = (n as f32 / N_SAMPLES_PER_TOKEN as f32).round_ties_even() as usize + 1;
    if token_count <= 2 {
        return Vec::new();
    }

    let k = (n as f64 * 0.001) as usize;
    if k == 0 {
        // Sub-1000-sample clips: upstream uses a quantile fallback we don't need
        // for real audio. Returning empty keeps callers on the "no mask" path.
        return Vec::new();
    }
    // `kth_largest` reorders its input, so derive the threshold from a copy and
    // keep `mag` in time order for interpolation.
    let threshold = kth_largest(&mut mag.clone(), k);

    if threshold < 1e-5 {
        return vec![0.0; token_count];
    }

    let denom = (threshold * 1.75).min(1.0);
    for v in &mut mag {
        *v /= denom;
    }

    interpolate_linear(&mag, token_count)
}

/// 1D suppression mask for silence, mirroring `wav2mask(audio, q_levels=20,
/// k_size=5)` on the `vad=False` path.
///
/// Returns `None` exactly where upstream returns `None`: when the loudness
/// envelope is empty (clip too short), or when no silence survives the 0.1 s
/// minimum-duration filter. When the quantized envelope is entirely silent the
/// returned mask is all `true` (everything suppressed), matching upstream's
/// `return ~mask`.
///
/// `true` marks tokens to suppress (silence); `false` marks speech. The mask
/// length equals the loudness/token count.
#[must_use]
pub fn wav2mask(audio: &[f32]) -> Option<Vec<bool>> {
    wav2mask_with(audio, 20, 5)
}

/// [`wav2mask`] with explicit `q_levels` / `k_size`, matching the upstream
/// keyword arguments. The defaults (`q_levels = 20`, `k_size = 5`) are what the
/// submate pipeline uses.
#[must_use]
pub fn wav2mask_with(audio: &[f32], q_levels: u32, k_size: usize) -> Option<Vec<bool>> {
    let loudness = audio2loudness(audio);
    if loudness.is_empty() {
        return None;
    }
    let size = loudness.len();

    // avg_pool1d(kernel=k_size, stride=1) over reflect-padded loudness.
    let pad = k_size / 2;
    let smoothed = if pad != 0 && pad < size {
        debug_assert!(k_size % 2 == 1, "kernel_size must be odd");
        avg_pool1d_reflect(&loudness, k_size)
    } else {
        loudness.clone()
    };

    // Quantize: round(x * q_levels); any non-zero bucket is "loud".
    let loud_mask: Vec<bool> = if q_levels != 0 {
        smoothed
            .iter()
            .map(|&x| (x * q_levels as f32).round_ties_even() != 0.0)
            .collect()
    } else {
        smoothed.iter().map(|&x| x != 0.0).collect()
    };

    // Entirely silent -> suppress everything (upstream `return ~mask`).
    if !loud_mask.iter().any(|&b| b) {
        return Some(vec![true; size]);
    }

    // Upstream extracts the *loud* runs from `loud_mask` (mask2timing returns
    // timings where the mask is True), keeps only runs longer than 0.1 s, and
    // rebuilds a loud mask via timing2mask before inverting it into silence.
    //
    // Each loud run [a, b] (inclusive) becomes a half-open timing [a, b + 1) in
    // token units; it survives when (end - start) / TOKENS_PER_SECOND > 0.1.
    // timing2mask then marks [a, b + 1] *inclusive* loud (a deliberate one-token
    // extension at the trailing edge, clamped to the array bound). The returned
    // suppression mask is the inverse: `true` where there is silence.
    let mut loud_rebuilt = vec![false; size];
    // Rebuild each kept loud run [start, end_excl) into `loud_rebuilt`, marking
    // [start, end_excl] *inclusive* (the one-token trailing extension), clamped
    // to the array. Runs are filtered in *seconds* as `end/50 - start/50 > 0.1`
    // computed in f64 — the two separate divisions are load-bearing, e.g.
    // 204/50 - 199/50 == 0.10000000000000009 > 0.1, so an exactly-0.1 s run is
    // kept.
    let mut mark_run = |start: usize, end_excl: usize| {
        let s_sec = start as f64 / TOKENS_PER_SECOND as f64;
        let e_sec = end_excl as f64 / TOKENS_PER_SECOND as f64;
        if e_sec - s_sec > 0.1 {
            let last = end_excl.min(size - 1);
            for v in loud_rebuilt.iter_mut().take(last + 1).skip(start) {
                *v = true;
            }
        }
    };
    let mut run_start: Option<usize> = None;
    for (i, &loud) in loud_mask.iter().enumerate() {
        match (loud, run_start) {
            (true, None) => run_start = Some(i),
            (false, Some(start)) => {
                mark_run(start, i); // end_excl = i (== b + 1)
                run_start = None;
            }
            _ => {}
        }
    }
    if let Some(start) = run_start {
        mark_run(start, size); // run extends to the final index
    }

    let mask: Vec<bool> = loud_rebuilt.iter().map(|&loud| !loud).collect();
    if !mask.iter().any(|&b| b) {
        // No silence survived: upstream returns None (no suppression).
        return None;
    }
    Some(mask)
}

/// The `k`-th largest value of `data` (1-indexed: `k = 1` -> the maximum),
/// matching `torch.topk(x, k)[-1]`. Reorders `data` in place.
fn kth_largest(data: &mut [f32], k: usize) -> f32 {
    debug_assert!(k >= 1 && k <= data.len());
    // Largest is index 0 once sorted descending; the k-th largest is index k-1.
    let idx = k - 1;
    data.select_nth_unstable_by(idx, |a, b| b.total_cmp(a));
    data[idx]
}

/// Linear interpolation to `out_size` points, reproducing torch
/// `F.interpolate(mode="linear", align_corners=False)`.
///
/// All index arithmetic and blending is done in `f32`, matching torch's CPU
/// `upsample_linear1d` kernel exactly (it computes the scale, source position,
/// and weights in the tensor's `f32` dtype). The source coordinate of output
/// index `i` is `(i + 0.5) * (n / out_size) - 0.5` clamped to `>= 0`; the result
/// is `v0 * (1 - frac) + v1 * frac` over the bracketing inputs.
fn interpolate_linear(src: &[f32], out_size: usize) -> Vec<f32> {
    let n = src.len();
    if n == 0 || out_size == 0 {
        return Vec::new();
    }
    if n == 1 {
        return vec![src[0]; out_size];
    }
    let scale = n as f32 / out_size as f32;
    let mut out = Vec::with_capacity(out_size);
    for i in 0..out_size {
        let pos = ((i as f32 + 0.5) * scale - 0.5).max(0.0);
        let i0 = pos.floor() as usize;
        let frac = pos - i0 as f32;
        let lo = i0.min(n - 1);
        let hi = (i0 + 1).min(n - 1);
        let v = src[lo] * (1.0 - frac) + src[hi] * frac;
        out.push(v);
    }
    out
}

/// `avg_pool1d(kernel_size=k, stride=1)` over reflect-padded input, matching
/// torch `avg_pool1d(F.pad(x, (p, p), "reflect"), kernel_size=k, stride=1)` with
/// `p = k / 2`. The output length equals the input length.
///
/// Reflect padding mirrors around the edge sample without repeating it, e.g.
/// `[a, b, c]` padded by 2 -> `[c, b, a, b, c, b, a]`. Each output is the mean of
/// the `k`-wide window centered on the corresponding input index.
fn avg_pool1d_reflect(src: &[f32], k: usize) -> Vec<f32> {
    let n = src.len();
    let pad = k / 2;
    let sample = |idx: isize| -> f32 {
        // Reflect index into [0, n - 1].
        let mut j = idx;
        if n == 1 {
            return src[0];
        }
        let span = (n - 1) as isize;
        loop {
            if j < 0 {
                j = -j;
            } else if j > span {
                j = 2 * span - j;
            } else {
                break;
            }
        }
        src[j as usize]
    };
    let mut out = Vec::with_capacity(n);
    for center in 0..n {
        let mut acc = 0.0f32;
        for off in 0..k {
            let idx = center as isize + off as isize - pad as isize;
            acc += sample(idx);
        }
        out.push(acc / k as f32);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interpolate_downsamples_like_torch() {
        // torch.nn.functional.interpolate([0..10], size=4, mode="linear",
        // align_corners=False) == [0.75, 3.25, 5.75, 8.25].
        let src: Vec<f32> = (0..10).map(|x| x as f32).collect();
        assert_eq!(interpolate_linear(&src, 4), vec![0.75, 3.25, 5.75, 8.25]);
    }

    #[test]
    fn interpolate_upsamples_with_edge_clamp() {
        // [1, 2, 3] -> size 6 clamps the negative-source first/last positions.
        let out = interpolate_linear(&[1.0, 2.0, 3.0], 6);
        assert_eq!(out, vec![1.0, 1.25, 1.75, 2.25, 2.75, 3.0]);
    }

    #[test]
    fn avg_pool_reflect_matches_torch() {
        // avg_pool1d(reflect-pad([10,20,30,40,50], 2), kernel=5, stride=1).
        let out = avg_pool1d_reflect(&[10.0, 20.0, 30.0, 40.0, 50.0], 5);
        assert_eq!(out, vec![22.0, 24.0, 30.0, 36.0, 38.0]);
    }

    #[test]
    fn kth_largest_picks_descending_rank() {
        // 1-indexed: the 2nd largest of [3, 1, 4, 1, 5] is 4.
        assert_eq!(kth_largest(&mut [3.0, 1.0, 4.0, 1.0, 5.0], 2), 4.0);
        assert_eq!(kth_largest(&mut [3.0, 1.0, 4.0, 1.0, 5.0], 1), 5.0);
    }

    #[test]
    fn silent_audio_suppresses_everything() {
        // Long, fully silent clip: every quantized bucket is 0, so upstream's
        // `return ~mask` suppresses all tokens.
        let audio = vec![0.0f32; 5000];
        let mask = wav2mask(&audio).expect("all-silent returns an all-true mask");
        assert!(mask.iter().all(|&b| b));
    }
}
