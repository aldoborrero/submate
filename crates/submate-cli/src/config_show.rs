//! Display formatting for `submate config show`.
//!
//! The command renders a table of *flattened, human-formatted, title-cased*
//! rows; the ordered `(setting, value)` rows are the contract. Three composable
//! steps:
//!
//! 1. [`format_value`] — leaf rendering.
//! 2. [`flatten_settings`] — depth-first flatten of the serde-JSON tree into
//!    ordered `(dotted_name, display)` rows.
//! 3. [`title_case_name`] — per-segment `replace('_', ' ').title()`, joined by
//!    `'.'`.
//!
//! Enums are already serialized to their string values in the JSON, so the
//! flatten walks the same tree.

use serde_json::Value;

/// Render a leaf config value for display.
///
/// Branch order is load-bearing:
/// * list -> `", "`-joined items, or `"(none)"` when empty;
/// * bool -> `"Yes"`/`"No"` (checked *before* the empty/None branch);
/// * empty string or null -> `"(not set)"`;
/// * else -> the scalar's string form.
///
/// An empty-string check must not match `0`/`0.0`, so numeric leaves render via
/// their own arm; the explicit number arm below preserves that.
fn format_value(value: &Value) -> String {
    match value {
        Value::Array(items) => {
            if items.is_empty() {
                "(none)".to_string()
            } else {
                items.iter().map(scalar_str).collect::<Vec<_>>().join(", ")
            }
        }
        Value::Bool(b) => {
            if *b {
                "Yes".to_string()
            } else {
                "No".to_string()
            }
        }
        Value::Null => "(not set)".to_string(),
        Value::String(s) => {
            if s.is_empty() {
                "(not set)".to_string()
            } else {
                s.clone()
            }
        }
        Value::Number(_) => scalar_str(value),
        // An object leaf never occurs here: `flatten_settings` recurses into
        // objects before reaching `format_value`. Fall back to the scalar form.
        Value::Object(_) => scalar_str(value),
    }
}

/// String form of a scalar for the leaf types that appear in a serialized
/// `Config`: strings verbatim, bools as `"True"`/`"False"`, numbers without
/// trailing-zero churn, null as `"None"`.
///
/// This is the list-item rendering, distinct from [`format_value`]'s top-level
/// branch logic (a bare `"True"` here would only be reached for a bool *inside a
/// list*, which the list branch renders the same way).
fn scalar_str(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Bool(b) => {
            if *b {
                "True".to_string()
            } else {
                "False".to_string()
            }
        }
        Value::Null => "None".to_string(),
        Value::Number(n) => n.to_string(),
        other => other.to_string(),
    }
}

/// Flatten a nested settings tree into ordered `(dotted_name, display)` rows.
///
/// Objects recurse depth-first, preserving field-declaration order (serde_json
/// preserves object insertion order when built from a `Serialize` struct).
/// Scalars and lists become a single row via [`format_value`].
fn flatten_settings(value: &Value, prefix: &str, rows: &mut Vec<(String, String)>) {
    match value {
        Value::Object(map) => {
            for (key, nested) in map {
                let name = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{prefix}.{key}")
                };
                flatten_settings(nested, &name, rows);
            }
        }
        _ => rows.push((prefix.to_string(), format_value(value))),
    }
}

/// Title-case a dotted setting name.
///
/// Each dotted segment is `replace('_', ' ')` then title-cased (see
/// [`python_title`]), and the segments are rejoined with `'.'`. Title-casing
/// uppercases the first letter of every run of alphabetic characters and
/// lowercases the rest, with any non-alphabetic character (including the
/// inserted space) acting as a word boundary.
fn title_case_name(dotted: &str) -> String {
    dotted
        .split('.')
        .map(|segment| python_title(&segment.replace('_', " ")))
        .collect::<Vec<_>>()
        .join(".")
}

/// Title-case a string: the first alphabetic char after any non-alphabetic char
/// is uppercased; other alphabetic chars are lowercased.
fn python_title(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_alpha = false;
    for ch in s.chars() {
        if ch.is_alphabetic() {
            if prev_alpha {
                out.extend(ch.to_lowercase());
            } else {
                out.extend(ch.to_uppercase());
            }
            prev_alpha = true;
        } else {
            out.push(ch);
            prev_alpha = false;
        }
    }
    out
}

/// Build the ordered `(setting, value)` display rows for `config show` from a
/// resolved `Config` serialized to serde-JSON.
///
/// This is the unit under test: flatten the tree, then title-case each dotted
/// name. Kept free of clap/IO so it is exercisable without the rest of the CLI.
pub fn config_show_rows(config_json: &Value) -> Vec<(String, String)> {
    let mut flat: Vec<(String, String)> = Vec::new();
    flatten_settings(config_json, "", &mut flat);
    flat.into_iter()
        .map(|(name, display)| (title_case_name(&name), display))
        .collect()
}

#[cfg(test)]
mod parity {
    use super::*;
    use ::parity::{EnvGuard, assert_json_eq, golden};
    use submate_config::Config;

    /// `[[setting, value], ...]` JSON, matching the goldens' shape.
    fn rows_to_value(rows: &[(String, String)]) -> Value {
        Value::Array(
            rows.iter()
                .map(|(name, display)| {
                    Value::Array(vec![
                        Value::String(name.clone()),
                        Value::String(display.clone()),
                    ])
                })
                .collect(),
        )
    }

    /// An override env exercising the [`format_value`] branches (plain string,
    /// numeric, bool).
    const OVERRIDE_ENV: &[(&str, &str)] = &[
        ("SUBMATE__WHISPER__MODEL", "large-v3"),
        ("SUBMATE__SERVER__PORT", "9123"),
        ("SUBMATE__DEBUG", "true"),
        ("SUBMATE__STABLE_TS__SUPPRESS_SILENCE", "false"),
    ];

    #[test]
    fn config_show_rows_defaults() {
        let cfg = Config::default();
        let json = serde_json::to_value(&cfg).expect("Config serializes to JSON");
        let actual = rows_to_value(&config_show_rows(&json));
        let expected = golden("cli/config_show.defaults.rows.json");
        assert_json_eq(&actual, &expected);
    }

    #[test]
    fn config_show_rows_overridden() {
        // Clear ambient `SUBMATE__*` and set the override env in a serialized,
        // isolated scope so resolution is reproducible and race-free.
        let _env = EnvGuard::set(OVERRIDE_ENV);

        let cfg = Config::from_env(None).expect("override env resolves into Config");
        let json = serde_json::to_value(&cfg).expect("Config serializes to JSON");
        let actual = rows_to_value(&config_show_rows(&json));
        let expected = golden("cli/config_show.overridden.rows.json");
        assert_json_eq(&actual, &expected);
    }
}
