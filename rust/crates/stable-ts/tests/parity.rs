//! Parity tests against golden fixtures captured from `stable_whisper.result`.
//!
//! Falsifier `parity::model_roundtrip`: parse the captured `to_dict()` JSON
//! (`stablets/clipA/00_raw.json`) into the ported [`WhisperResult`] and
//! re-emit it via [`WhisperResult::to_dict`]; the result must equal the golden
//! JSON value exactly (`parity::assert_json_eq` does a structural, float-aware
//! comparison — the capture writes sorted-key/pretty JSON, but the Rust side
//! compares parsed `serde_json::Value`s, so formatting is irrelevant).

use ::parity::{assert_json_eq, golden};
use stable_ts::WhisperResult;

#[test]
fn model_roundtrip() {
    let raw = golden("stablets/clipA/00_raw.json");
    let result = WhisperResult::from_value(&raw);
    let actual = result.to_dict();
    assert_json_eq(&actual, &raw);
}

/// Suppress-silence DSP parity against goldens captured from
/// `stable_whisper.stabilization.nonvad`. Tests live in this `parity` module so
/// `cargo test -p stable-ts parity::wav2mask` (the backlog falsifier) selects
/// them by path.
mod parity {
    use ::parity::{assert_f32_close, load_f32};

    /// `audio2loudness(clipA/audio.f32)` must match the captured loudness
    /// envelope within `1e-6` (deterministic DSP, no ML model).
    #[test]
    fn audio2loudness() {
        let audio = load_f32("stablets/clipA/audio.f32");
        let golden = load_f32("stablets/clipA/loudness.f32");
        let actual = stable_ts::audio2loudness(&audio);
        assert_f32_close(&actual, &golden, 1e-6);
    }

    /// `wav2mask(clipA/audio.f32)` must match the captured suppression mask
    /// (0/1 per token) within `1e-6`. The golden was captured from
    /// `stable_whisper.stabilization.nonvad.wav2mask` with the pipeline's
    /// defaults (`q_levels = 20`, `k_size = 5`).
    #[test]
    fn wav2mask() {
        let audio = load_f32("stablets/clipA/audio.f32");
        let golden = load_f32("stablets/clipA/mask.f32");
        let mask = stable_ts::wav2mask(&audio).expect("clipA has silence to suppress");
        let actual: Vec<f32> = mask.iter().map(|&b| if b { 1.0 } else { 0.0 }).collect();
        assert_f32_close(&actual, &golden, 1e-6);
    }
}
