//! Parity tests against golden fixtures captured from `stable_whisper.result`.
//!
//! Falsifier `parity::model_roundtrip`: parse the captured `to_dict()` JSON
//! (`stablets/clipA/00_raw.json`) into the ported [`WhisperResult`] and
//! re-emit it via [`WhisperResult::to_dict`]; the result must equal the golden
//! JSON value exactly (`parity::assert_json_eq` does a structural, float-aware
//! comparison — the capture writes sorted-key/pretty JSON, but the Rust side
//! compares parsed `serde_json::Value`s, so formatting is irrelevant).

use parity::{assert_json_eq, golden};
use stable_ts::WhisperResult;

#[test]
fn model_roundtrip() {
    let raw = golden("stablets/clipA/00_raw.json");
    let result = WhisperResult::from_value(&raw);
    let actual = result.to_dict();
    assert_json_eq(&actual, &raw);
}
