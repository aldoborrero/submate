//! Parity tests against golden fixtures captured from `stable_whisper.result`.
//!
//! Each test parses a captured `to_dict()` JSON golden into the ported
//! [`WhisperResult`] and re-emits it via [`WhisperResult::to_dict`]; the result
//! must equal the golden JSON value exactly (`parity::assert_json_eq` does a
//! structural, float-aware comparison — the capture writes sorted-key/pretty
//! JSON, but the Rust side compares parsed `serde_json::Value`s, so formatting
//! is irrelevant). The goldens span the empty (`00_raw`), regroup-staged
//! (`01_regroup_*`), and populated-`nonspeech_sections` (`02_suppress`) shapes.

use parity::{assert_json_eq, golden};
use stable_ts::WhisperResult;

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
