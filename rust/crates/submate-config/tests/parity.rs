//! Config parity falsifiers.
//!
//! * `parity::defaults` — a default-constructed [`Config`] serializes to exactly
//!   the captured Python golden `config/defaults.resolved.json`, pinning every
//!   field name and default value to the Pydantic model.
//! * `parity::env_nesting` — `config/nested.env` resolved through
//!   [`Config::from_env`] equals `config/nested.resolved.json`, pinning the
//!   `SUBMATE__` prefix, the `__` nesting delimiter, and env-coercion of scalar
//!   field types (port, enums).
//!
//! Object key ordering is irrelevant: both sides are compared as
//! `serde_json::Value` (BTreeMap-backed).

use parity::{assert_json_eq, fixture_path, golden};
use submate_config::Config;

#[test]
fn defaults() {
    let cfg = Config::default();
    let actual = serde_json::to_value(&cfg).expect("Config serializes to JSON");
    let expected = golden("config/defaults.resolved.json");
    assert_json_eq(&actual, &expected);
}

/// Parse a `.env` fixture into `(key, value)` pairs.
///
/// Deliberately minimal — the fixtures are plain `KEY=value` lines with no
/// quoting or comments, so a `split_once('=')` per non-blank line is enough and
/// avoids pulling in a dotenv crate just to feed the jail.
fn parse_env(rel: &str) -> Vec<(String, String)> {
    let raw = std::fs::read_to_string(fixture_path(rel))
        .unwrap_or_else(|e| panic!("missing env fixture {rel}: {e}"));
    raw.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| {
            let (k, v) = line
                .split_once('=')
                .unwrap_or_else(|| panic!("malformed env line (no '='): {line:?}"));
            (k.to_string(), v.to_string())
        })
        .collect()
}

#[test]
fn env_nesting() {
    // `Jail` sets the env vars in an isolated, serialized scope so this test
    // neither leaks `SUBMATE__*` into the process nor races other tests.
    figment::Jail::expect_with(|jail| {
        // Start from an empty environment so ambient `SUBMATE__*` vars on the
        // developer/CI machine can't leak into resolution. `clear_env` records
        // every removed var and restores it when the jail drops.
        jail.clear_env();
        for (key, value) in parse_env("config/nested.env") {
            jail.set_env(key, value);
        }

        let cfg = Config::from_env(None).expect("env resolves into Config");
        let actual = serde_json::to_value(&cfg).expect("Config serializes to JSON");
        let expected = golden("config/nested.resolved.json");
        assert_json_eq(&actual, &expected);
        Ok(())
    });
}
