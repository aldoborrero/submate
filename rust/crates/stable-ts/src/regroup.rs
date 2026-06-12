//! Port of stable-ts's regroup DSL parser (`WhisperResult.parse_regroup_algo`).
//!
//! The regroup algorithm is configured by a single string such as
//! `"cm_sl=84_sl=42++++++1"`. [`parse_regroup_algo`] turns that string into an
//! ordered list of [`RegroupOp`]s — each a method name plus the keyword
//! arguments to call it with — which the apply stage (B2) then binds to the
//! actual regroup methods on [`crate::WhisperResult`].
//!
//! ## Grammar
//!
//! The string is split into operations on `_`. Each operation is
//! `method[=arg+arg+...]`:
//!
//! * the part before the first `=` is the two/three-letter method code (`cm`,
//!   `sl`, `sg`, …);
//! * the part after the first `=` (if any) is the argument list, split on `+`;
//! * each argument is coerced by [`str_to_valid_type`] (empty → "absent",
//!   `12` → int, `1.5` → float, `a/b` → list, otherwise the raw string);
//! * arguments are then zipped positionally onto the method's parameter names,
//!   and any "absent" argument is dropped — so `sl=42++++++1` binds
//!   `max_chars=42` and `newline=1`, skipping the five empty middle slots.
//!
//! The special code `da` ("default algorithm") expands in place to the upstream
//! default operation string.
//!
//! ## Parity shape
//!
//! [`RegroupOp::to_value`] / [`ops_to_value`] reproduce the capture
//! (`fixtures/capture/capture_stablets.py`) which records each op as
//! `{"method": <full method name>, "kwargs": {...}}`. Parsing the configured
//! regroup string and emitting that list reproduces the
//! `stablets/regroup_parse.json` golden exactly (see the
//! `parity::regroup_parse` falsifier).

use serde_json::{Map, Number, Value};

/// The upstream default expansion for the `da` ("default algorithm") code.
///
/// Mirrors `parse_regroup_algo`'s `default_calls`; substituted in place of any
/// `da` token before the per-operation parse.
const DEFAULT_ALGO: &str = "cm_sp=,* /，_sg=.5_mg=.3+3_sp=.* /。/?/？";

/// One parsed regroup operation: the resolved method name and its keyword args.
///
/// `kwargs` preserves the order arguments were bound in (the method's parameter
/// order), and each value is the [`str_to_valid_type`]-coerced JSON form so the
/// emitted parity JSON matches Python's `to_dict()`-style numbers exactly.
#[derive(Debug, Clone, PartialEq)]
pub struct RegroupOp {
    /// The full method name (e.g. `clamp_max`, `split_by_length`).
    pub method: String,
    /// Bound keyword arguments, in parameter order, absent slots dropped.
    pub kwargs: Vec<(String, Value)>,
}

impl RegroupOp {
    /// Emit `{"method": ..., "kwargs": {...}}`, matching the capture script.
    #[must_use]
    pub fn to_value(&self) -> Value {
        let mut kwargs = Map::new();
        for (k, v) in &self.kwargs {
            kwargs.insert(k.clone(), v.clone());
        }
        let mut op = Map::new();
        op.insert("kwargs".to_string(), Value::Object(kwargs));
        op.insert("method".to_string(), Value::String(self.method.clone()));
        Value::Object(op)
    }
}

/// Emit a parsed op list as the `[{method, kwargs}, ...]` parity JSON value.
#[must_use]
pub fn ops_to_value(ops: &[RegroupOp]) -> Value {
    Value::Array(ops.iter().map(RegroupOp::to_value).collect())
}

/// Error returned when the regroup string names an unknown method code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownMethod(pub String);

impl std::fmt::Display for UnknownMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} is not one of the available methods: {:?}",
            self.0,
            METHODS.iter().map(|(code, _, _)| *code).collect::<Vec<_>>()
        )
    }
}

impl std::error::Error for UnknownMethod {}

/// The method-code table: `(code, full_name, positional_parameter_names)`.
///
/// Mirrors the `methods` dict in `parse_regroup_algo`, in declaration order.
/// The parameter names are the methods' positional parameters (after `self`),
/// used to bind the positional `+`-split arguments to keyword names.
const METHODS: &[(&str, &str, &[&str])] = &[
    ("sg", "split_by_gap", &["max_gap", "lock", "newline"]),
    (
        "sp",
        "split_by_punctuation",
        &["punctuation", "lock", "newline", "min_words", "min_chars", "min_dur"],
    ),
    (
        "sl",
        "split_by_length",
        &["max_chars", "max_words", "even_split", "force_len", "lock", "include_lock", "newline"],
    ),
    (
        "sd",
        "split_by_duration",
        &["max_dur", "even_split", "force_len", "lock", "include_lock", "newline"],
    ),
    (
        "mg",
        "merge_by_gap",
        &["min_gap", "max_words", "max_chars", "is_sum_max", "lock", "newline"],
    ),
    (
        "mp",
        "merge_by_punctuation",
        &["punctuation", "max_words", "max_chars", "is_sum_max", "lock", "newline"],
    ),
    ("ms", "merge_all_segments", &[]),
    ("cm", "clamp_max", &["medium_factor", "max_dur", "clip_start", "verbose"]),
    ("us", "unlock_all_segments", &[]),
    ("l", "lock", &["startswith", "endswith", "right", "left", "case_sensitive", "strip"]),
    ("rw", "remove_word", &["word", "reassign_ids", "verbose"]),
    ("rs", "remove_segment", &["segment", "reassign_ids", "verbose"]),
    (
        "rp",
        "remove_repetition",
        &["max_words", "case_sensitive", "strip", "ignore_punctuations", "extend_duration", "verbose"],
    ),
    (
        "rws",
        "remove_words_by_str",
        &["words", "case_sensitive", "strip", "ignore_punctuations", "min_prob", "filters", "verbose"],
    ),
    (
        "fg",
        "fill_in_gaps",
        &["other_result", "min_gap", "case_sensitive", "strip", "ignore_punctuations", "verbose"],
    ),
    ("p", "pad", &["start_pad", "end_pad", "max_dur", "max_end", "word_level"]),
];

/// Coerce one DSL argument to its value, mirroring `utils.str_to_valid_type`.
///
/// * empty string → [`None`] ("absent" — dropped when binding kwargs);
/// * contains `/` → a list value, where each `/`-segment that contains `*` is
///   itself split into a sub-list on `*` (used by the punctuation methods);
/// * otherwise an `int` if it has no `.`, a `float` if it does, falling back to
///   the raw string when the numeric parse fails.
#[must_use]
pub fn str_to_valid_type(val: &str) -> Option<Value> {
    if val.is_empty() {
        return None;
    }
    if val.contains('/') {
        let list = val
            .split('/')
            .map(|a| {
                if a.contains('*') {
                    Value::Array(a.split('*').map(|s| Value::String(s.to_string())).collect())
                } else {
                    Value::String(a.to_string())
                }
            })
            .collect();
        return Some(Value::Array(list));
    }
    if val.contains('.') {
        if let Ok(f) = val.parse::<f64>() {
            return Some(Number::from_f64(f).map_or_else(|| Value::String(val.to_string()), Value::Number));
        }
    } else if let Ok(i) = val.parse::<i64>() {
        return Some(Value::Number(Number::from(i)));
    }
    Some(Value::String(val.to_string()))
}

/// Parse a regroup algorithm string into an ordered list of [`RegroupOp`]s.
///
/// Mirrors `WhisperResult.parse_regroup_algo`: split on `_`, expand any `da`
/// token to [`DEFAULT_ALGO`], then for each operation split off the method code
/// and `+`-split/coerce its arguments and bind them positionally onto the
/// method's parameter names (dropping "absent" empty slots).
///
/// Returns [`UnknownMethod`] if any operation names a code not in [`METHODS`].
pub fn parse_regroup_algo(regroup_algo: &str) -> Result<Vec<RegroupOp>, UnknownMethod> {
    if regroup_algo.is_empty() {
        return Ok(Vec::new());
    }

    let raw_calls: Vec<&str> = regroup_algo.split('_').collect();
    let calls: Vec<&str> = if raw_calls.contains(&"da") {
        raw_calls
            .into_iter()
            .flat_map(|method| {
                if method == "da" {
                    DEFAULT_ALGO.split('_').collect::<Vec<_>>()
                } else {
                    vec![method]
                }
            })
            .collect()
    } else {
        raw_calls
    };

    let mut operations = Vec::with_capacity(calls.len());
    for call in calls {
        let (method, args) = match call.split_once('=') {
            Some((m, a)) => (m, a),
            None => (call, ""),
        };
        let Some((_, name, params)) = METHODS.iter().find(|(code, _, _)| *code == method) else {
            return Err(UnknownMethod(method.to_string()));
        };

        let values: Vec<Option<Value>> =
            if args.is_empty() { Vec::new() } else { args.split('+').map(str_to_valid_type).collect() };

        let kwargs = params
            .iter()
            .zip(values)
            .filter_map(|(name, value)| value.map(|v| ((*name).to_string(), v)))
            .collect();

        operations.push(RegroupOp { method: (*name).to_string(), kwargs });
    }

    Ok(operations)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn empty_algo_is_empty() {
        assert_eq!(parse_regroup_algo("").unwrap(), Vec::new());
    }

    #[test]
    fn unknown_method_errors() {
        assert_eq!(parse_regroup_algo("zz").unwrap_err(), UnknownMethod("zz".to_string()));
    }

    #[test]
    fn configured_algo_matches_capture_shape() {
        let ops = parse_regroup_algo("cm_sl=84_sl=42++++++1").unwrap();
        assert_eq!(
            ops_to_value(&ops),
            json!([
                {"method": "clamp_max", "kwargs": {}},
                {"method": "split_by_length", "kwargs": {"max_chars": 84}},
                {"method": "split_by_length", "kwargs": {"max_chars": 42, "newline": 1}},
            ])
        );
    }

    #[test]
    fn str_to_valid_type_coercions() {
        assert_eq!(str_to_valid_type(""), None);
        assert_eq!(str_to_valid_type("84"), Some(json!(84)));
        assert_eq!(str_to_valid_type("0.5"), Some(json!(0.5)));
        assert_eq!(str_to_valid_type("abc"), Some(json!("abc")));
        assert_eq!(str_to_valid_type("a/b"), Some(json!(["a", "b"])));
        assert_eq!(str_to_valid_type(",* /，"), Some(json!([[",", " "], "，"])));
    }

    #[test]
    fn da_expands_to_default_algo() {
        let direct = parse_regroup_algo(DEFAULT_ALGO).unwrap();
        let expanded = parse_regroup_algo("da").unwrap();
        assert_eq!(direct, expanded);
        // First op of the default expansion is `clamp_max`.
        assert_eq!(expanded[0].method, "clamp_max");
    }
}
