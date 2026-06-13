//! Display formatting for `submate config show` (ports the pure-data layer of
//! `submate/cli/commands/config.py`).
//!
//! The Python command renders a Rich table of *flattened, human-formatted,
//! title-cased* rows. The Rich `Table` chrome (borders/colors) is out of scope;
//! the ordered `(setting, value)` rows are the contract. Three composable steps,
//! ported byte-for-byte:
//!
//! 1. [`format_value`] — leaf rendering (`_format_value`).
//! 2. [`flatten_settings`] — depth-first flatten of the serde-JSON tree into
//!    ordered `(dotted_name, display)` rows (`_flatten_settings`).
//! 3. [`title_case_name`] — per-segment `replace('_', ' ').title()`, joined by
//!    `'.'`.
//!
//! Enums are already serialized to their `.value` strings in the JSON (the
//! Python side dumps with `mode="json"`), so the flatten walks the same tree.

use serde_json::Value;

/// Render a leaf config value for display (ports `_format_value`).
///
/// Branch order is load-bearing and matches Python exactly:
/// * list -> `", "`-joined items, or `"(none)"` when empty;
/// * bool -> `"Yes"`/`"No"` (checked *before* the empty/None branch);
/// * empty string or null -> `"(not set)"`;
/// * else -> the scalar's string form.
///
/// Python's `value == ""` does not match `0`/`0.0`, so numeric leaves render via
/// `str(value)`; the explicit number arm below preserves that.
fn format_value(value: &Value) -> String {
    match value {
        Value::Array(items) => {
            if items.is_empty() {
                "(none)".to_string()
            } else {
                items
                    .iter()
                    .map(scalar_str)
                    .collect::<Vec<_>>()
                    .join(", ")
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

/// String form of a scalar, mirroring Python `str(value)` for the leaf types
/// that appear in a serialized `Config`: strings verbatim, bools as
/// `"True"`/`"False"`, numbers without trailing-zero churn, null as `"None"`.
///
/// This is the list-item / `str(value)` rendering, distinct from
/// [`format_value`]'s top-level branch logic (a bare `"True"` here would only be
/// reached for a bool *inside a list*, which `_format_value` renders the same way
/// because the list branch calls `str(item)`).
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

/// Flatten a nested settings tree into ordered `(dotted_name, display)` rows
/// (ports `_flatten_settings`).
///
/// Objects recurse depth-first, preserving field-declaration order (serde_json
/// preserves object insertion order when built from a `Serialize` struct, which
/// matches Python dict order = Pydantic field order). Scalars and lists become a
/// single row via [`format_value`].
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

/// Title-case a dotted setting name (ports the display-name map).
///
/// Each dotted segment is `replace('_', ' ')` then `str.title()`-cased, and the
/// segments are rejoined with `'.'`. Python `str.title()` uppercases the first
/// letter of every run of alphabetic characters and lowercases the rest, with
/// any non-alphabetic character (including the inserted space) acting as a word
/// boundary.
fn title_case_name(dotted: &str) -> String {
    dotted
        .split('.')
        .map(|segment| python_title(&segment.replace('_', " ")))
        .collect::<Vec<_>>()
        .join(".")
}

/// Mirror Python's `str.title()`: the first alphabetic char after any
/// non-alphabetic char is uppercased; other alphabetic chars are lowercased.
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
    use ::parity::{assert_json_eq, golden, EnvGuard};
    use submate_config::Config;

    /// `[[setting, value], ...]` JSON, matching the goldens' shape.
    fn rows_to_value(rows: &[(String, String)]) -> Value {
        Value::Array(
            rows.iter()
                .map(|(name, display)| {
                    Value::Array(vec![Value::String(name.clone()), Value::String(display.clone())])
                })
                .collect(),
        )
    }

    /// The override env from `capture_cli_config.py`, exercising every
    /// `_format_value` branch (plain string, numeric, populated list, bool).
    const OVERRIDE_ENV: &[(&str, &str)] = &[
        ("SUBMATE__WHISPER__MODEL", "large-v3"),
        ("SUBMATE__SERVER__PORT", "9123"),
        ("SUBMATE__SUBTITLE__SKIP_SUBTITLE_LANGUAGES", "eng|spa"),
        ("SUBMATE__SUBTITLE__SKIP_UNKNOWN_LANGUAGE", "true"),
    ];

    /// Expand `queue.db_path`'s `${XDG_DATA_HOME}` template the way the Python
    /// `config show` command does at config-build time.
    ///
    /// The submate-config Rust port deliberately keeps `db_path` as the
    /// *unexpanded* template `${XDG_DATA_HOME}/subgen/queue.db` (pinned by its own
    /// `config/defaults.resolved.json` golden), whereas Pydantic resolved it to an
    /// absolute path via `get_xdg_data_home()` (`$XDG_DATA_HOME` or
    /// `~/.local/share`) before `model_dump`. That db_path resolution divergence
    /// is out of scope here — `config_show_rows` is a pure renderer of whatever
    /// the serialized `Config` holds — so the test expands the template the same
    /// way Python does, keeping the *transform under test* exact while letting the
    /// comparison reproduce the machine-derived golden value. `xdg` is the
    /// `get_xdg_data_home()` base, captured from the ambient env *before* any
    /// `Jail::clear_env`.
    fn expand_db_path(json: &mut Value, xdg: &str) {
        if let Some(db) = json
            .get("queue")
            .and_then(|q| q.get("db_path"))
            .and_then(Value::as_str)
            .map(|s| s.replace("${XDG_DATA_HOME}", xdg))
        {
            json["queue"]["db_path"] = Value::String(db);
        }
    }

    /// `get_xdg_data_home()`: `$XDG_DATA_HOME`, else `$HOME/.local/share`.
    fn xdg_data_home() -> String {
        std::env::var("XDG_DATA_HOME").unwrap_or_else(|_| {
            let home = std::env::var("HOME").expect("HOME is set");
            format!("{home}/.local/share")
        })
    }

    #[test]
    fn config_show_rows_defaults() {
        let xdg = xdg_data_home();
        let cfg = Config::default();
        let mut json = serde_json::to_value(&cfg).expect("Config serializes to JSON");
        expand_db_path(&mut json, &xdg);
        let actual = rows_to_value(&config_show_rows(&json));
        let expected = golden("cli/config_show.defaults.rows.json");
        assert_json_eq(&actual, &expected);
    }

    #[test]
    fn config_show_rows_overridden() {
        // Capture the XDG base from the ambient env up front.
        let xdg = xdg_data_home();
        // Clear ambient `SUBMATE__*` and set the override env in a serialized,
        // isolated scope (see `parity::EnvGuard`) so resolution is reproducible
        // and race-free; the previous environment is restored when `_env` drops.
        let _env = EnvGuard::set(OVERRIDE_ENV);

        let cfg = Config::from_env(None).expect("override env resolves into Config");
        let mut json = serde_json::to_value(&cfg).expect("Config serializes to JSON");
        expand_db_path(&mut json, &xdg);
        let actual = rows_to_value(&config_show_rows(&json));
        let expected = golden("cli/config_show.overridden.rows.json");
        assert_json_eq(&actual, &expected);
    }
}
