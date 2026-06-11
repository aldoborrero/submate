//! Falsifier `parity::defaults`: a default-constructed [`Config`] serializes
//! to exactly the captured Python golden `config/defaults.resolved.json`.
//!
//! This pins the schema layer: every field name and default value must match
//! the Pydantic model byte-for-byte. Object key ordering is irrelevant because
//! both sides are compared as `serde_json::Value` (BTreeMap-backed).

use parity::{assert_json_eq, golden};
use submate_config::Config;

#[test]
fn defaults() {
    let cfg = Config::default();
    let actual = serde_json::to_value(&cfg).expect("Config serializes to JSON");
    let expected = golden("config/defaults.resolved.json");
    assert_json_eq(&actual, &expected);
}
