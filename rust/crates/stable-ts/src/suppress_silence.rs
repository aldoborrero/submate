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
    let mut abs: Vec<f32> = audio.iter().map(|x| x.abs()).collect();

    // threshold = the k-th largest |sample|, k = int(n * 0.001).
    // (torch.topk(k)[-1]); for k == 0 upstream uses quantile(0.999) instead.
    let k = ((n as f64) * 0.001) as usize;
    let threshold = if k != 0 {
        // Descending; kth largest sits at index k-1. NaN-free audio, so total
        // order. Quickselect partitions `abs` at k-1 in O(n) instead of an
        // O(n log n) sort; the element it lands there is exactly what a full
        // descending sort would place there. `abs` is reordered in place, so
        // `scaled` below is rebuilt from `audio` rather than read positionally.
        let (_, kth, _) = abs.select_nth_unstable_by(k - 1, |a, b| b.partial_cmp(a).unwrap());
        *kth
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
    // Rebuild from `audio`: `abs` was reordered by the quickselect above, and
    // `interpolate_linear` reads its input positionally.
    let scaled: Vec<f32> = audio.iter().map(|x| x.abs() / divisor).collect();

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

/// `stable_whisper.stabilization.utils.mask2timing`: convert a per-token silence
/// mask into the `(silent_starts, silent_ends)` pair of *seconds*.
///
/// Each contiguous `true` run becomes one half-open `[start, end)` range whose
/// token indices are divided by [`TOKENS_PER_SECOND`] (upstream `silent_ends` is
/// the index just past the run, so the division yields the half-open span in
/// seconds). Returns `None` for an empty or all-`false` mask, mirroring the
/// upstream `if ... not silence_mask.any() ... return` guard.
///
/// The division stays in `f64` to match numpy's `silent_starts / TOKENS_PER_SECOND`
/// (the same width the per-word [`suppress`] comparisons and
/// `update_nonspeech_sections` rounding rely on).
#[must_use]
pub fn mask2timing(silence_mask: &[bool]) -> Option<(Vec<f64>, Vec<f64>)> {
    if silence_mask.is_empty() || !silence_mask.iter().any(|&b| b) {
        return None;
    }
    let (starts, ends): (Vec<f64>, Vec<f64>) = mask_runs(silence_mask)
        .into_iter()
        .map(|(s, e)| (s as f64 / TOKENS_PER_SECOND, e as f64 / TOKENS_PER_SECOND))
        .unzip();
    Some((starts, ends))
}

/// `stable_whisper.stabilization.nonvad.audio2timings`: the full non-VAD silence
/// detector, `mask2timing(wav2mask(audio))`.
///
/// Returns the `(silent_starts, silent_ends)` second ranges fed into per-word
/// [`suppress`], or `None` when there is no suppressible silence (either
/// [`wav2mask`] or [`mask2timing`] returns nothing).
#[must_use]
pub fn audio2timings(audio: &[f32]) -> Option<(Vec<f64>, Vec<f64>)> {
    mask2timing(&wav2mask(audio)?)
}

/// `stable_whisper.default.DEFAULT_VALUES['min_word_dur']`.
pub const DEFAULT_MIN_WORD_DUR: f64 = 0.1;

/// `stable_whisper.default.DEFAULT_KWARGS['append_punctuations']` — the trailing
/// punctuation that, when it ends a word, flips `keep_end` to `false` so the
/// *end* timestamp is anchored to the punctuation instead of being pushed in.
const APPEND_PUNCTUATIONS: &str = "\"'.。,，!！?？:：”)]}、」";

/// Apply the non-VAD silence map to every word's timing, mirroring
/// `WhisperResult.suppress_silence(..., word_level=True, use_word_position=True)`
/// as it runs inside `transcribe_stable` (the submate default).
///
/// For each segment that has words, each word is clipped against the
/// `(silent_starts, silent_ends)` ranges via [`suppress`] with `min_word_dur`
/// and `nonspeech_error`, and `keep_end` derived from the word's position:
/// upstream's `keep_end = not (word[-1] in append_punctuations or i == len(words))`
/// — the last word in a segment (1-indexed `i == len`) and any word ending in
/// append punctuation keep their *start* (`keep_end = false`); all others keep
/// their *end*.
///
/// Segments without words fall through to the segment-level [`suppress`]
/// (`keep_end = true`), matching `Segment.suppress_silence`'s `else` branch.
/// This mutates the result in place; the caller is responsible for
/// `update_nonspeech_sections` (the verbatim `nonspeech_sections` payload).
pub fn suppress_silence(
    result: &mut crate::WhisperResult,
    silent_starts: &[f64],
    silent_ends: &[f64],
    min_word_dur: f64,
    nonspeech_error: f64,
) {
    for segment in &mut result.segments {
        match segment.words.as_mut() {
            Some(words) if !words.is_empty() => {
                let n = words.len();
                for (i, word) in words.iter_mut().enumerate() {
                    let ends_in_punct = word
                        .word
                        .chars()
                        .next_back()
                        .is_some_and(|c| APPEND_PUNCTUATIONS.contains(c));
                    let keep_end = !(ends_in_punct || i + 1 == n);
                    let mut span = WordSpan::new(word.start(), word.end());
                    suppress(&mut span, silent_starts, silent_ends, min_word_dur, nonspeech_error, Some(keep_end));
                    word.set_start(span.start);
                    word.set_end(span.end);
                }
            }
            _ => {
                // Wordless segment: adjust the segment's default start/end with
                // the upstream `keep_end=True` default.
                let mut span = WordSpan::new(segment.start(), segment.end());
                suppress(&mut span, silent_starts, silent_ends, min_word_dur, nonspeech_error, Some(true));
                segment.set_default_span(span.start, span.end);
            }
        }
    }
}

/// `WhisperResult.set_current_as_orig(keep_orig=False)`: overwrite `ori_dict`
/// with the current serialized state, where that snapshot's own nested
/// `ori_dict` is empty (`keep_orig=False`).
///
/// `transcribe_stable` calls this immediately after the suppress stage, so the
/// captured `02_suppress.json` carries a *suppressed* `ori_dict` (populated
/// `nonspeech_sections`, clipped word timings, empty inner `ori_dict`) rather
/// than the pre-suppress raw one. Run this after [`suppress_silence`] /
/// [`update_nonspeech_sections`] to reproduce that shape.
pub fn set_current_as_orig(result: &mut crate::WhisperResult) {
    // to_dict(keep_orig=False) serializes with an empty inner `ori_dict`; emptying
    // it before the snapshot reproduces that, and the snapshot then *becomes* the
    // new `ori_dict`.
    result.ori_dict = serde_json::Value::Object(serde_json::Map::new());
    result.ori_dict = result.to_dict();
}

/// `WhisperResult.update_nonspeech_sections`: store the detected silence ranges
/// verbatim as the `nonspeech_sections` list of `{start, end}` dicts, each
/// timestamp `round(.., 3)` (numpy `float64` rounding, half-to-even).
///
/// Mirrors the call `transcribe_stable` makes right after `suppress_silence`, so
/// running [`suppress_silence`] then this reproduces the populated
/// `02_suppress.json` shape end to end.
pub fn update_nonspeech_sections(
    result: &mut crate::WhisperResult,
    silent_starts: &[f64],
    silent_ends: &[f64],
) {
    let sections: Vec<serde_json::Value> = silent_starts
        .iter()
        .zip(silent_ends)
        .map(|(&s, &e)| {
            let mut m = serde_json::Map::new();
            m.insert("start".into(), json_number(round3(s)));
            m.insert("end".into(), json_number(round3(e)));
            serde_json::Value::Object(m)
        })
        .collect();
    result.nonspeech_sections = serde_json::Value::Array(sections);
}

/// JSON number for a finite `f64`, falling back to `Null` (only timings here,
/// always finite).
fn json_number(v: f64) -> serde_json::Value {
    serde_json::Number::from_f64(v).map_or(serde_json::Value::Null, serde_json::Value::Number)
}

/// A mutable `(start, end)` pair standing in for the upstream `result_obj`
/// (a `WordTiming` or wordless `Segment`) that [`suppress`] mutates.
///
/// Upstream every `result_obj.start = ...` / `.end = ...` goes through the
/// `WordTiming`/`Segment` setter, which applies `_round_timestamp` (round-3,
/// `if not ts` falsy guard). Because later steps in [`suppress`] *read* those
/// fields back, the rounding has to happen on each write here too — so the
/// setters call [`crate::round_timestamp`] and the fields are only mutated
/// through them.
struct WordSpan {
    start: f64,
    end: f64,
}

impl WordSpan {
    fn new(start: f64, end: f64) -> Self {
        WordSpan { start, end }
    }

    fn set_start(&mut self, val: f64) {
        self.start = crate::round_timestamp(val);
    }

    fn set_end(&mut self, val: f64) {
        self.end = crate::round_timestamp(val);
    }
}

/// Port of `stable_whisper.stabilization.suppress_silence` (the per-object
/// timestamp clip), operating on one [`WordSpan`].
///
/// Mirrors the upstream control flow exactly:
/// 1. no-op when there are no silences or the span is already `<= min_word_dur`;
/// 2. **start overlap** (`keep_end`): if a silence brackets the start
///    (`silent_starts <= start < silent_ends <= end`), snap `start` to that
///    silence's end (capped at `round(end - min_word_dur, 3)`);
/// 3. **end overlap** (`!keep_end`): symmetric snap of `end` to a silence start;
/// 4. **nonspeech tolerance**: when exactly one silence is fully contained in the
///    span, shrink whichever side's relative error is within `nonspeech_error`.
///
/// `round_half_even` (Python `round`) is reproduced for the `min_word_dur` caps;
/// the comparisons stay in `f64` to match numpy.
fn suppress(
    span: &mut WordSpan,
    silent_starts: &[f64],
    silent_ends: &[f64],
    min_word_dur: f64,
    nonspeech_error: f64,
    keep_end: Option<bool>,
) {
    debug_assert_eq!(silent_starts.len(), silent_ends.len());
    if silent_starts.is_empty() || (span.end - span.start) <= min_word_dur {
        return;
    }

    // start_overlaps: (keep_end is None or keep_end) and the first silence with
    // silent_starts <= start < silent_ends <= end.
    if keep_end.is_none() || keep_end == Some(true) {
        if let Some(i) = (0..silent_starts.len()).find(|&i| {
            silent_starts[i] <= span.start && span.start < silent_ends[i] && silent_ends[i] <= span.end
        }) {
            let new_start = silent_ends[i];
            span.set_start(new_start.min(round3(span.end - min_word_dur)));
            if (span.end - span.start) <= min_word_dur {
                return;
            }
        }
    }

    // end_overlaps: (not keep_end) and the first silence with
    // start <= silent_starts < end <= silent_ends.
    if keep_end == Some(false) {
        if let Some(i) = (0..silent_starts.len()).find(|&i| {
            span.start <= silent_starts[i] && silent_starts[i] < span.end && span.end <= silent_ends[i]
        }) {
            let new_end = silent_starts[i];
            span.set_end(new_end.max(round3(span.start + min_word_dur)));
            if (span.end - span.start) <= min_word_dur {
                return;
            }
        }
    }

    if nonspeech_error == 0.0 {
        return;
    }

    // matches: silences fully contained in [start, end]. Upstream requires
    // exactly one; otherwise it returns without adjusting.
    let matches: Vec<usize> = (0..silent_starts.len())
        .filter(|&i| span.start <= silent_starts[i] && span.end >= silent_ends[i])
        .collect();
    if matches.len() != 1 {
        return;
    }
    let idx = matches[0];
    let silence_start = silent_starts[idx];
    let silence_end = silent_ends[idx];
    let silent_duration = silence_end - silence_start;
    let start_error = (silence_start - span.start) / silent_duration;
    let end_error = (span.end - silence_end) / silent_duration;

    let resolved_keep_end = keep_end.unwrap_or(start_error <= end_error);
    let start_within = start_error <= nonspeech_error;
    let end_within = end_error <= nonspeech_error;
    if !(start_within || end_within) {
        return;
    }
    if resolved_keep_end {
        span.set_start(silence_end.min(round3(span.end - min_word_dur)));
    } else {
        span.set_end(silence_start.max(round3(span.start + min_word_dur)));
    }
}

/// Python `round(x, 3)` (round-half-to-even) for the `min_word_dur` caps inside
/// [`suppress`]. Unlike [`crate::round_timestamp`] this does not special-case
/// falsy values: upstream applies a bare `round(..., 3)` here.
fn round3(x: f64) -> f64 {
    let scaled = x * 1000.0;
    scaled.round_ties_even() / 1000.0
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

    #[test]
    fn mask2timing_converts_runs_to_seconds() {
        // Runs (1,3) and (4,5) -> divided by TOKENS_PER_SECOND (50).
        let m = [false, true, true, false, true];
        let (s, e) = mask2timing(&m).expect("has silence");
        assert_eq!(s, vec![1.0 / 50.0, 4.0 / 50.0]);
        assert_eq!(e, vec![3.0 / 50.0, 5.0 / 50.0]);
    }

    #[test]
    fn mask2timing_none_for_empty_or_silent() {
        assert!(mask2timing(&[]).is_none());
        assert!(mask2timing(&[false, false]).is_none());
    }

    #[test]
    fn suppress_keep_end_snaps_start_to_silence_end() {
        // Word [0.0, 0.5], silence [0.0, 0.2) at the start, keep_end=true:
        // start snaps to the silence end (0.2), capped at end - min_word_dur.
        let mut span = WordSpan::new(0.0, 0.5);
        suppress(&mut span, &[0.0], &[0.2], 0.1, 0.1, Some(true));
        assert_eq!(span.start, 0.2);
        assert_eq!(span.end, 0.5);
    }

    #[test]
    fn suppress_not_keep_end_snaps_end_to_silence_start() {
        // Word [0.0, 0.5], silence [0.3, 0.5) at the end, keep_end=false:
        // end snaps to the silence start (0.3), floored at start + min_word_dur.
        let mut span = WordSpan::new(0.0, 0.5);
        suppress(&mut span, &[0.3], &[0.5], 0.1, 0.1, Some(false));
        assert_eq!(span.start, 0.0);
        assert_eq!(span.end, 0.3);
    }

    #[test]
    fn suppress_min_word_dur_caps_start_clip() {
        // A silence that would shrink the word below min_word_dur caps the new
        // start at round(end - min_word_dur, 3) = 0.4.
        let mut span = WordSpan::new(0.0, 0.5);
        suppress(&mut span, &[0.0], &[0.49], 0.1, 0.1, Some(true));
        assert_eq!(span.start, 0.4);
    }

    #[test]
    fn suppress_noop_when_no_silence_or_too_short() {
        let mut span = WordSpan::new(0.0, 0.5);
        suppress(&mut span, &[], &[], 0.1, 0.1, Some(true));
        assert_eq!((span.start, span.end), (0.0, 0.5));

        // Word already <= min_word_dur: untouched.
        let mut short = WordSpan::new(0.0, 0.1);
        suppress(&mut short, &[0.0], &[0.05], 0.1, 0.1, Some(true));
        assert_eq!((short.start, short.end), (0.0, 0.1));
    }

    #[test]
    fn suppress_nonspeech_tolerance_shrinks_contained_silence() {
        // Silence [0.4, 0.5) fully inside word [0.0, 0.5], keep_end=true.
        // start_extra = 0.4, end_extra = 0.0, silent_dur = 0.1.
        // start_error = 4.0 (> 0.1), end_error = 0.0 (<= 0.1) -> adjust:
        // keep_end -> start = min(silence_end, round(end - min_word_dur)).
        let mut span = WordSpan::new(0.0, 0.5);
        suppress(&mut span, &[0.4], &[0.5], 0.1, 0.1, Some(true));
        assert_eq!(span.start, 0.4);
        assert_eq!(span.end, 0.5);
    }
}
