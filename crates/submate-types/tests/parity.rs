//! Falsifier `parity::enum_values`: every enum variant serializes to its exact
//! recorded string, and parses back, in both `Display`/`FromStr` and serde
//! directions.
//!
//! The golden `types/enum_values.json` records the enums as
//! `{EnumName: {VARIANT_NAME: value}}`. Each Rust variant is paired with its
//! recorded variant name here so a divergence in either set of strings — or a
//! missing/extra variant on either side — fails loudly.

use std::collections::BTreeMap;
use std::fmt::Display;
use std::str::FromStr;

use fixtures::{assert_json_eq, golden};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use strum::IntoEnumIterator;

use submate_types::{
    Device, LanguageNamingType, TranscriptionTask, TranslationBackend, WhisperModel,
};

/// Assert one enum against its golden `{variant_name: value}` table.
///
/// `pairs` lists `(variant_name, rust_variant)` for the full enum. We rebuild
/// the golden table from the Rust side and compare byte-for-byte against the
/// recorded JSON, then round-trip every string through `FromStr` and serde to
/// prove both decode directions agree too.
fn check_enum<T>(enum_name: &str, pairs: &[(&str, T)])
where
    T: Copy
        + PartialEq
        + Display
        + FromStr
        + Serialize
        + for<'de> Deserialize<'de>
        + IntoEnumIterator
        + std::fmt::Debug,
    <T as FromStr>::Err: std::fmt::Debug,
{
    let golden_table = golden("types/enum_values.json");
    let expected = &golden_table[enum_name];
    assert!(
        expected.is_object(),
        "golden has no object for enum {enum_name}: {golden_table}"
    );

    // The pairs must cover exactly the enum's variants — no more, no less.
    assert_eq!(
        pairs.len(),
        T::iter().count(),
        "{enum_name}: test pairs ({}) do not cover all variants ({})",
        pairs.len(),
        T::iter().count()
    );

    let mut built = serde_json::Map::new();
    for (py_name, variant) in pairs {
        // Display (strum) and serde must emit the same string.
        let via_display = variant.to_string();
        let via_serde = serde_json::to_value(variant).unwrap();
        assert_eq!(
            Value::String(via_display.clone()),
            via_serde,
            "{enum_name}::{py_name}: Display {via_display:?} != serde {via_serde:?}"
        );

        // Both decode directions round-trip back to the same variant.
        let parsed: T = via_display.parse().unwrap_or_else(|e| {
            panic!("{enum_name}::{py_name}: FromStr({via_display:?}) failed: {e:?}")
        });
        assert_eq!(
            parsed, *variant,
            "{enum_name}::{py_name}: FromStr did not round-trip"
        );
        let de: T = serde_json::from_value(json!(via_display)).unwrap();
        assert_eq!(
            de, *variant,
            "{enum_name}::{py_name}: serde did not round-trip"
        );

        built.insert((*py_name).to_string(), Value::String(via_display));
    }

    assert_json_eq(&Value::Object(built), expected);
}

/// All enums in the golden must be accounted for by a `check_enum` call below.
const COVERED_ENUMS: &[&str] = &[
    "WhisperModel",
    "Device",
    "TranscriptionTask",
    "LanguageNamingType",
    "TranslationBackend",
];

#[test]
fn enum_values() {
    check_enum(
        "WhisperModel",
        &[
            ("TINY", WhisperModel::Tiny),
            ("TINY_EN", WhisperModel::TinyEn),
            ("BASE", WhisperModel::Base),
            ("BASE_EN", WhisperModel::BaseEn),
            ("SMALL", WhisperModel::Small),
            ("SMALL_EN", WhisperModel::SmallEn),
            ("MEDIUM", WhisperModel::Medium),
            ("MEDIUM_EN", WhisperModel::MediumEn),
            ("LARGE", WhisperModel::Large),
            ("LARGE_V1", WhisperModel::LargeV1),
            ("LARGE_V2", WhisperModel::LargeV2),
            ("LARGE_V3", WhisperModel::LargeV3),
        ],
    );

    check_enum(
        "Device",
        &[
            ("CPU", Device::Cpu),
            ("CUDA", Device::Cuda),
            ("VULKAN", Device::Vulkan),
            ("AUTO", Device::Auto),
        ],
    );

    check_enum(
        "TranscriptionTask",
        &[
            ("TRANSCRIBE", TranscriptionTask::Transcribe),
            ("TRANSLATE", TranscriptionTask::Translate),
        ],
    );

    check_enum(
        "LanguageNamingType",
        &[
            ("ISO_639_1", LanguageNamingType::Iso6391),
            ("ISO_639_2_T", LanguageNamingType::Iso6392T),
            ("ISO_639_2_B", LanguageNamingType::Iso6392B),
            ("NAME", LanguageNamingType::Name),
            ("NATIVE", LanguageNamingType::Native),
        ],
    );

    check_enum(
        "TranslationBackend",
        &[
            ("OLLAMA", TranslationBackend::Ollama),
            ("CLAUDE", TranslationBackend::Claude),
            ("OPENAI", TranslationBackend::Openai),
            ("GEMINI", TranslationBackend::Gemini),
        ],
    );
}

/// Guard against the golden gaining an enum the tests forgot to cover.
#[test]
fn no_uncovered_enums_in_golden() {
    let golden_table = golden("types/enum_values.json");
    let obj = golden_table
        .as_object()
        .expect("golden enum_values.json is not an object");
    let covered: BTreeMap<&str, ()> = COVERED_ENUMS.iter().map(|n| (*n, ())).collect();
    for name in obj.keys() {
        assert!(
            covered.contains_key(name.as_str()),
            "golden enum {name:?} is not covered by enum_values test"
        );
    }
    assert_eq!(obj.len(), COVERED_ENUMS.len(), "enum count mismatch");
}
