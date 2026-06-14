//! Parity tests against golden fixtures captured.

use parity::golden;
use serde_json::Value;
use submate_lang::LanguageCode;

/// Python enum member name for a variant. The Rust `Debug` name matches the
/// Python member name for every variant except the absent one, which Python
/// spells `NONE` and Rust spells `None`.
fn member_name(lang: LanguageCode) -> String {
    match lang {
        LanguageCode::None => "NONE".to_string(),
        other => format!("{other:?}"),
    }
}

/// Map a Python enum member name (e.g. `"ENGLISH"`, `"NONE"`) to its variant.
fn variant_by_name(name: &str) -> LanguageCode {
    if name == "NONE" {
        return LanguageCode::None;
    }
    LanguageCode::all()
        .find(|v| format!("{v:?}") == name)
        .unwrap_or_else(|| panic!("unknown enum member name {name:?}"))
}

/// A JSON string field, or `None` for JSON null.
fn opt_str(v: &Value) -> Option<&str> {
    match v {
        Value::Null => None,
        Value::String(s) => Some(s.as_str()),
        other => panic!("expected string or null, got {other}"),
    }
}

#[test]
fn lang_conversions() {
    let rows = golden("lang/lang_conversions.json");
    let rows = rows.as_array().expect("fixture is an array");

    // Exact count: 101 languages + NONE.
    assert_eq!(rows.len(), 102, "fixture row count");

    for row in rows {
        let name = row["name"].as_str().expect("name field");
        let lang = variant_by_name(name);

        // Forward direction: enum -> codes/names.
        assert_eq!(lang.to_iso_639_1(), opt_str(&row["iso_639_1"]), "{name}: iso_639_1");
        assert_eq!(lang.to_iso_639_2_t(), opt_str(&row["iso_639_2_t"]), "{name}: iso_639_2_t");
        assert_eq!(lang.to_iso_639_2_b(), opt_str(&row["iso_639_2_b"]), "{name}: iso_639_2_b");
        assert_eq!(lang.to_name(true), opt_str(&row["name_en"]), "{name}: name_en");
        assert_eq!(lang.to_name(false), opt_str(&row["name_native"]), "{name}: name_native");

        // Reverse direction: each accessor string resolves back via from_string.
        let from_iso_639_1 = lang
            .to_iso_639_1()
            .map(|code| member_name(LanguageCode::from_string(Some(code))));
        assert_eq!(
            from_iso_639_1.as_deref(),
            opt_str(&row["from_iso_639_1"]),
            "{name}: from_iso_639_1 round-trip"
        );

        let from_name_en = lang
            .to_name(true)
            .map(|n| member_name(LanguageCode::from_string(Some(n))));
        assert_eq!(
            from_name_en.as_deref(),
            opt_str(&row["from_name_en"]),
            "{name}: from_name_en round-trip"
        );
    }
}
