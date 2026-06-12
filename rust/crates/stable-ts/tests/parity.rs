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
use stable_ts::{
    apply_regroup_op, audio2timings, ops_to_value, parse_regroup_algo, set_current_as_orig,
    suppress_silence, update_nonspeech_sections, WhisperResult, DEFAULT_MIN_WORD_DUR,
};

/// The submate config regroup string (see `fixtures/capture/capture_stablets.py`).
const REGROUP: &str = "cm_sl=84_sl=42++++++1";

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

/// B2 apply falsifier: each staged regroup op, applied *in isolation* to a fresh
/// `WhisperResult` parsed from `00_raw.json`, must reproduce its golden exactly.
///
/// This mirrors `capture_stablets.py`, which rebuilds a fresh result from
/// `raw_dict` per op (so stages don't compound) and dumps `to_dict()`. The op
/// list comes from the same `REGROUP` string `parse_regroup_algo` parses, and
/// the golden filename per op is `01_regroup_<i>_<method>.json`.
#[test]
fn regroup_apply() {
    let raw = golden("stablets/clipA/00_raw.json");
    let ops = parse_regroup_algo(REGROUP).expect("known methods");

    for (i, op) in ops.iter().enumerate() {
        let golden_name = format!("stablets/clipA/01_regroup_{i}_{}.json", op.method);
        let expected = golden(&golden_name);

        let mut result = WhisperResult::from_value(&raw);
        apply_regroup_op(&mut result, op).expect("staged op is applicable in B2");

        assert_json_eq(&result.to_dict(), &expected);
    }
}

/// C2 apply falsifier: the full non-VAD suppress-silence stage.
///
/// `capture_stablets.py` produces `02_suppress.json` by re-running the engine
/// with `regroup=False, suppress_silence=True` — i.e. it applies suppression to
/// the *unregrouped* result (`00_raw`), not to a regroup stage. So this rebuilds
/// `00_raw`, derives the silence ranges from `audio.f32` via
/// [`audio2timings`] (= `mask2timing(wav2mask(..))`), applies the per-word
/// [`suppress_silence`] and [`update_nonspeech_sections`] with the same defaults
/// `transcribe_stable` uses (`min_word_dur=0.1`, `nonspeech_error=0.1`,
/// `word_level=True`, `use_word_position=True`), and checks the result against
/// the golden — pinning both the clipped word timings and the populated
/// `nonspeech_sections`.
#[test]
fn suppress() {
    let raw = golden("stablets/clipA/00_raw.json");
    let expected = golden("stablets/clipA/02_suppress.json");
    let audio = load_f32("stablets/clipA/audio.f32");

    let (starts, ends) = audio2timings(&audio).expect("clipA has suppressible silence");

    let mut result = WhisperResult::from_value(&raw);
    suppress_silence(&mut result, &starts, &ends, DEFAULT_MIN_WORD_DUR, 0.1);
    update_nonspeech_sections(&mut result, &starts, &ends);
    // transcribe_stable snapshots the suppressed state into `ori_dict` right
    // after the stage, so the golden's `ori_dict` is itself suppressed.
    set_current_as_orig(&mut result);

    assert_json_eq(&result.to_dict(), &expected);
}
