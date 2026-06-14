//! Falsifier `parity::queue_enum_values`: every `OutputFormat` / `SkipReason`
//! variant serializes to the exact Python `.value` string captured in
//! `queue/enum_values.json`.
//!
//! The golden uses its **own** file (not `types/enum_values.json`, whose
//! `no_uncovered_enums_in_golden` guard requires exactly the six `types.py`
//! enums). It is shaped `{EnumName: {PYTHON_VARIANT_NAME: value}}`; each Rust
//! variant is paired with its Python variant name so a divergence in either
//! set of strings — or a missing/extra variant — fails loudly.

use std::collections::BTreeMap;

use parity::{assert_json_eq, golden};
use serde::Serialize;
use serde_json::Value;
use strum::IntoEnumIterator;

use submate_queue::{OutputFormat, SkipReason};

/// Assert one enum against its golden `{python_name: value}` table.
///
/// `pairs` lists `(python_variant_name, rust_variant)` for the full enum,
/// covering exactly its variants. We rebuild the table from the Rust side
/// (serde `Serialize`) and compare byte-for-byte against the captured JSON.
fn check_enum<T>(enum_name: &str, pairs: &[(&str, T)])
where
    T: Copy + Serialize + IntoEnumIterator,
{
    let golden_table = golden("queue/enum_values.json");
    let expected = &golden_table[enum_name];
    assert!(
        expected.is_object(),
        "golden has no object for enum {enum_name}: {golden_table}"
    );

    assert_eq!(
        pairs.len(),
        T::iter().count(),
        "{enum_name}: test pairs ({}) do not cover all variants ({})",
        pairs.len(),
        T::iter().count()
    );

    let mut built = serde_json::Map::new();
    for (py_name, variant) in pairs {
        let via_serde = serde_json::to_value(variant).unwrap();
        assert!(
            via_serde.is_string(),
            "{enum_name}::{py_name}: serde did not produce a string: {via_serde}"
        );
        built.insert((*py_name).to_string(), via_serde);
    }

    assert_json_eq(&Value::Object(built), expected);
}

/// All enums in the golden must be accounted for by a `check_enum` call below.
const COVERED_ENUMS: &[&str] = &["OutputFormat", "SkipReason"];

#[test]
fn queue_enum_values() {
    check_enum(
        "OutputFormat",
        &[
            ("SRT", OutputFormat::Srt),
            ("VTT", OutputFormat::Vtt),
            ("TXT", OutputFormat::Txt),
            ("JSON", OutputFormat::Json),
        ],
    );

    check_enum(
        "SkipReason",
        &[
            ("NOT_SKIPPED", SkipReason::NotSkipped),
            ("TARGET_SUBTITLE_EXISTS", SkipReason::TargetSubtitleExists),
            (
                "EXTERNAL_SUBTITLE_EXISTS",
                SkipReason::ExternalSubtitleExists,
            ),
            (
                "INTERNAL_SUBTITLE_LANGUAGE_EXISTS",
                SkipReason::InternalSubtitleLanguageExists,
            ),
            (
                "SUBTITLE_LANGUAGE_IN_SKIP_LIST",
                SkipReason::SubtitleLanguageInSkipList,
            ),
            (
                "AUDIO_LANGUAGE_IN_SKIP_LIST",
                SkipReason::AudioLanguageInSkipList,
            ),
            ("UNKNOWN_LANGUAGE", SkipReason::UnknownLanguage),
            (
                "NO_PREFERRED_AUDIO_LANGUAGE",
                SkipReason::NoPreferredAudioLanguage,
            ),
            ("LRC_FILE_EXISTS", SkipReason::LrcFileExists),
            (
                "LANGUAGE_NOT_SET_BUT_SUBTITLES_EXIST",
                SkipReason::LanguageNotSetButSubtitlesExist,
            ),
        ],
    );
}

/// Guard against the golden gaining a queue enum the Rust port forgot to cover.
#[test]
fn no_uncovered_enums_in_golden() {
    let golden_table = golden("queue/enum_values.json");
    let obj = golden_table
        .as_object()
        .expect("golden queue/enum_values.json is not an object");
    let covered: BTreeMap<&str, ()> = COVERED_ENUMS.iter().map(|n| (*n, ())).collect();
    for name in obj.keys() {
        assert!(
            covered.contains_key(name.as_str()),
            "golden enum {name:?} is not covered by queue_enum_values test"
        );
    }
    assert_eq!(obj.len(), COVERED_ENUMS.len(), "enum count mismatch");
}

/// Reproduce `tests/test_queue.py::test_output_format_from_value_normalizes`
/// plus the `.extension` behavior the Python `OutputFormat` exposes.
#[test]
fn from_value_coercion() {
    assert_eq!(
        OutputFormat::from_value(Some("vtt"), None),
        OutputFormat::Vtt
    );
    assert_eq!(
        OutputFormat::from_value(Some("json"), None),
        OutputFormat::Json
    );
    assert_eq!(
        OutputFormat::from_value(Some("nonsense"), None),
        OutputFormat::Srt
    );
    assert_eq!(
        OutputFormat::from_value(Some("nonsense"), Some(OutputFormat::Txt)),
        OutputFormat::Txt
    );
    // `None` input behaves like an unknown string.
    assert_eq!(OutputFormat::from_value(None, None), OutputFormat::Srt);
    assert_eq!(
        OutputFormat::from_value(None, Some(OutputFormat::Txt)),
        OutputFormat::Txt
    );

    assert_eq!(OutputFormat::Srt.extension(), ".srt");
}
