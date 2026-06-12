//! Parity tests against golden fixtures captured from `stable_whisper.result`.
//!
//! Each test parses a captured `to_dict()` JSON golden into the ported
//! [`WhisperResult`] and re-emits it via [`WhisperResult::to_dict`]; the result
//! must equal the golden JSON value exactly (`parity::assert_json_eq` does a
//! structural, float-aware comparison — the capture writes sorted-key/pretty
//! JSON, but the Rust side compares parsed `serde_json::Value`s, so formatting
//! is irrelevant). The goldens span the empty (`00_raw`), regroup-staged
//! (`01_regroup_*`), and populated-`nonspeech_sections` (`02_suppress`) shapes.

use parity::{assert_f32_close, assert_json_eq, golden, load_f32};
use stable_ts::{ops_to_value, parse_regroup_algo, WhisperResult};

#[test]
fn model_roundtrip() {
    let raw = golden("stablets/clipA/00_raw.json");
    let result = WhisperResult::from_value(&raw);
    let actual = result.to_dict();
    assert_json_eq(&actual, &raw);
}

/// Same roundtrip as `model_roundtrip`, but over `02_suppress.json` — the only
/// model-shaped golden whose `nonspeech_sections` is non-empty (27
/// `{"start", "end"}` dicts) and whose segments carry post-`suppress_silence`
/// word timings. Guards against a regression that drops, reorders, or re-types
/// the verbatim `nonspeech_sections` value or mis-derives `text`/`segments` for
/// the populated case.
#[test]
fn suppress_roundtrip() {
    let raw = golden("stablets/clipA/02_suppress.json");
    let actual = WhisperResult::from_value(&raw).to_dict();
    assert_json_eq(&actual, &raw);
}

/// The three intermediate `regroup` stages share the same `to_dict()` shape and
/// must roundtrip identically; this pins each stage against its captured golden.
#[test]
fn regroup_stage_roundtrip() {
    for name in [
        "stablets/clipA/01_regroup_0_clamp_max.json",
        "stablets/clipA/01_regroup_1_split_by_length.json",
        "stablets/clipA/01_regroup_2_split_by_length.json",
    ] {
        let raw = golden(name);
        let actual = WhisperResult::from_value(&raw).to_dict();
        assert_json_eq(&actual, &raw);
    }
}

/// `audio2loudness(audio.f32)` must match the Python-captured `loudness.f32`
/// (dumped straight from `stable_whisper.stabilization.nonvad.audio2loudness`)
/// within `1e-6`. This pins the abs -> 0.1%-topk threshold -> normalize ->
/// `f32` linear-interpolate-to-token-count chain against torch's exact output.
#[test]
fn audio2loudness() {
    let audio = load_f32("stablets/clipA/audio.f32");
    let golden = load_f32("stablets/clipA/loudness.f32");
    let actual = stable_ts::audio2loudness(&audio).expect("clipA is long enough for loudness");
    assert_f32_close(&actual, &golden, 1e-6);
}

/// `wav2mask(audio.f32)` must match the Python-captured `mask.f32` (the bool
/// mask `nonvad.wav2mask` returns, dumped as `0.0`/`1.0`) within `1e-6`. This
/// pins the avg-pool (`k=5`, reflect) -> quantize (`q_levels=20`) -> timing
/// roundtrip -> invert chain.
#[test]
fn wav2mask() {
    let audio = load_f32("stablets/clipA/audio.f32");
    let golden = load_f32("stablets/clipA/mask.f32");
    let mask = stable_ts::wav2mask(&audio).expect("clipA has both silence and audible audio");
    let actual: Vec<f32> = mask.iter().map(|&b| if b { 1.0 } else { 0.0 }).collect();
    assert_f32_close(&actual, &golden, 1e-6);
}

/// `parse_regroup_algo` produces (`stablets/regroup_parse.json`).
#[test]
fn regroup_parse() {
    let golden_ops = golden("stablets/regroup_parse.json");
    let ops = parse_regroup_algo("cm_sl=84_sl=42++++++1").expect("known methods");
    assert_json_eq(&ops_to_value(&ops), &golden_ops);
}
