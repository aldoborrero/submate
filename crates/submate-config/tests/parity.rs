//! Config parity falsifiers.
//!
//! * `parity::defaults` — a default-constructed [`Config`] serializes to exactly
//!   the golden `config/defaults.resolved.json`, pinning every field name and
//!   default value.
//! * `parity::env_nesting` — `config/nested.env` resolved through
//!   [`Config::from_env`] equals `config/nested.resolved.json`, pinning the
//!   `SUBMATE__` prefix, the `__` nesting delimiter, and env-coercion of scalar
//!   field types (port, enums).
//! * `parity::validators` — `config/validators.env` resolved through
//!   [`Config::from_env`] equals `config/validators.resolved.json`, pinning the
//!   field coercions: pipe-separated lists split on `'|'`, the whisper decode
//!   knobs (`beam_size` → `u32`, `initial_prompt` → `String`), and the
//!   `custom_regroup` string passthrough (a non-disabling pattern stays a string).
//!
//! Object key ordering is irrelevant: both sides are compared as
//! `serde_json::Value` (BTreeMap-backed).

use parity::{EnvGuard, assert_json_eq, fixture_path, golden};
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
    // Clear ambient `SUBMATE__*` and set the test vars in a serialized, isolated
    // scope (see `parity::EnvGuard`) so this test neither leaks `SUBMATE__*` into
    // the process nor races other env-driven tests; restored when `_env` drops.
    let vars = parse_env("config/nested.env");
    let pairs: Vec<(&str, &str)> = vars.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    let _env = EnvGuard::set(&pairs);

    let cfg = Config::from_env(None).expect("env resolves into Config");
    let actual = serde_json::to_value(&cfg).expect("Config serializes to JSON");
    let expected = golden("config/nested.resolved.json");
    assert_json_eq(&actual, &expected);
}

#[test]
fn validators() {
    let vars = parse_env("config/validators.env");
    let pairs: Vec<(&str, &str)> = vars.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    let _env = EnvGuard::set(&pairs);

    let cfg = Config::from_env(None).expect("env resolves into Config");
    let actual = serde_json::to_value(&cfg).expect("Config serializes to JSON");
    let expected = golden("config/validators.resolved.json");
    assert_json_eq(&actual, &expected);
}
