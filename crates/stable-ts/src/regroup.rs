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
//! [`RegroupOp::to_value`] / [`ops_to_value`] emit each op as
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

// ---------------------------------------------------------------------------
// B2 apply stage: bind a parsed `RegroupOp` to the regroup method it names and
// run it against a `WhisperResult`. The split/merge apply family that parses in
// B1 is implemented here — `clamp_max`, `split_by_length`, `split_by_duration`,
// `split_by_gap`, `split_by_punctuation`, `merge_by_gap`, `merge_by_punctuation`,
// and `merge_all_segments` — along with the segment-split/merge helpers they
// share. Other method codes parse fine (B1) but are not yet runnable here and
// return `UnsupportedMethod`.
// ---------------------------------------------------------------------------

use crate::model::{Segment, WhisperResult, WordTiming};

/// Error returned when [`apply_regroup_op`] is handed a method that B2 parses
/// but does not yet execute (everything outside the split/merge family —
/// `clamp_max`/`split_by_length`/`split_by_duration`/`split_by_gap`/
/// `split_by_punctuation`/`merge_by_gap`/`merge_by_punctuation`/
/// `merge_all_segments`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsupportedMethod(pub String);

impl std::fmt::Display for UnsupportedMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "regroup method {} is parsed but not yet applied (B2)", self.0)
    }
}

impl std::error::Error for UnsupportedMethod {}

/// Apply every op in a parsed regroup list to `result`, in order.
///
/// Mirrors the loop `WhisperResult.regroup` runs over `parse_regroup_algo`'s
/// output: each op is a method name plus the bound kwargs, called on the
/// result in turn. Returns [`UnsupportedMethod`] for any op B2 doesn't yet run.
pub fn apply_regroup(result: &mut WhisperResult, ops: &[RegroupOp]) -> Result<(), UnsupportedMethod> {
    for op in ops {
        apply_regroup_op(result, op)?;
    }
    Ok(())
}

/// Apply a single parsed op to `result`, dispatching on its method name and
/// reading its bound kwargs by name (absent kwargs fall back to the method's
/// upstream default).
pub fn apply_regroup_op(result: &mut WhisperResult, op: &RegroupOp) -> Result<(), UnsupportedMethod> {
    match op.method.as_str() {
        "clamp_max" => {
            // Defaults from `WhisperResult.clamp_max`: medium_factor=2.5,
            // max_dur=None, clip_start=None, verbose=False.
            let medium_factor = op.kwarg_f64("medium_factor").unwrap_or(Some(2.5));
            let max_dur = op.kwarg_f64("max_dur").unwrap_or(None);
            let clip_start = op.kwarg_bool("clip_start").unwrap_or(None);
            clamp_max(result, medium_factor, max_dur, clip_start);
            Ok(())
        }
        "split_by_length" => {
            // Defaults from `WhisperResult.split_by_length`: max_chars=None,
            // max_words=None, even_split=True, force_len=False, lock=False,
            // include_lock=False, newline=False.
            let max_chars = op.kwarg_usize("max_chars").unwrap_or(None);
            let max_words = op.kwarg_usize("max_words").unwrap_or(None);
            let even_split = op.kwarg_bool("even_split").unwrap_or(Some(true)).unwrap_or(true);
            let force_len = op.kwarg_bool("force_len").unwrap_or(Some(false)).unwrap_or(false);
            let lock = op.kwarg_bool("lock").unwrap_or(Some(false)).unwrap_or(false);
            let include_lock = op.kwarg_bool("include_lock").unwrap_or(Some(false)).unwrap_or(false);
            let newline = op.kwarg_bool("newline").unwrap_or(Some(false)).unwrap_or(false);
            split_by_length(
                result,
                SplitByLength { max_chars, max_words, even_split, force_len, lock, include_lock, newline },
            );
            Ok(())
        }
        "split_by_duration" => {
            // Defaults from `WhisperResult.split_by_duration`: max_dur=None,
            // even_split=True, force_len=False, lock=False, include_lock=False,
            // newline=False. `max_dur` is the lone required positional upstream;
            // when absent the parsed `sd` op simply carries no `max_dur` kwarg,
            // mirroring the `sl` arm's optional-`max_chars` handling.
            let max_dur = op.kwarg_value("max_dur");
            let even_split = op.kwarg_bool("even_split").unwrap_or(Some(true)).unwrap_or(true);
            let force_len = op.kwarg_bool("force_len").unwrap_or(Some(false)).unwrap_or(false);
            let lock = op.kwarg_bool("lock").unwrap_or(Some(false)).unwrap_or(false);
            let include_lock = op.kwarg_bool("include_lock").unwrap_or(Some(false)).unwrap_or(false);
            let newline = op.kwarg_bool("newline").unwrap_or(Some(false)).unwrap_or(false);
            split_by_duration(
                result,
                SplitByDuration { max_dur, even_split, force_len, lock, include_lock, newline },
            );
            Ok(())
        }
        "split_by_gap" => {
            // Defaults from `WhisperResult.split_by_gap`: max_gap=0.1, lock=False,
            // newline=False. `max_gap` is kept as the raw coerced JSON value so the
            // history string reproduces the DSL form (e.g. `0.5`) exactly, mirroring
            // the `split_by_duration` arm's handling of `max_dur`.
            let max_gap = op.kwarg_value("max_gap");
            let lock = op.kwarg_bool("lock").unwrap_or(Some(false)).unwrap_or(false);
            let newline = op.kwarg_bool("newline").unwrap_or(Some(false)).unwrap_or(false);
            split_by_gap(result, SplitByGap { max_gap, lock, newline });
            Ok(())
        }
        "split_by_punctuation" => {
            // Defaults from `WhisperResult.split_by_punctuation`: lock=False,
            // newline=False, min_words=None, min_chars=None, min_dur=None.
            // `punctuation` is required upstream; absent here leaves the op a no-op.
            let punctuation = op.kwarg_value("punctuation");
            let lock = op.kwarg_bool("lock").unwrap_or(Some(false)).unwrap_or(false);
            let newline = op.kwarg_bool("newline").unwrap_or(Some(false)).unwrap_or(false);
            let min_words = op.kwarg_usize("min_words").unwrap_or(None);
            let min_chars = op.kwarg_usize("min_chars").unwrap_or(None);
            let min_dur = op.kwarg_f64("min_dur").unwrap_or(None);
            split_by_punctuation(
                result,
                SplitByPunctuation { punctuation, lock, newline, min_words, min_chars, min_dur },
            );
            Ok(())
        }
        "merge_by_gap" => {
            // Defaults from `WhisperResult.merge_by_gap`: min_gap=0.1,
            // max_words=None, max_chars=None, is_sum_max=False, lock=False,
            // newline=False. `min_gap` is kept as the raw coerced JSON value so
            // the history string reproduces the DSL form (e.g. `0.3`) exactly,
            // mirroring the `split_by_gap` arm's handling of `max_gap`.
            let min_gap = op.kwarg_value("min_gap");
            let max_words = op.kwarg_usize("max_words").unwrap_or(None);
            let max_chars = op.kwarg_usize("max_chars").unwrap_or(None);
            let is_sum_max = op.kwarg_bool("is_sum_max").unwrap_or(Some(false)).unwrap_or(false);
            let lock = op.kwarg_bool("lock").unwrap_or(Some(false)).unwrap_or(false);
            let newline = op.kwarg_bool("newline").unwrap_or(Some(false)).unwrap_or(false);
            merge_by_gap(result, MergeByGap { min_gap, max_words, max_chars, is_sum_max, lock, newline });
            Ok(())
        }
        "merge_by_punctuation" => {
            // Defaults from `WhisperResult.merge_by_punctuation`: max_words=None,
            // max_chars=None, is_sum_max=False, lock=False, newline=False.
            // `punctuation` is required upstream; absent here leaves the op a
            // no-op (mirroring the `split_by_punctuation` arm).
            let punctuation = op.kwarg_value("punctuation");
            let max_words = op.kwarg_usize("max_words").unwrap_or(None);
            let max_chars = op.kwarg_usize("max_chars").unwrap_or(None);
            let is_sum_max = op.kwarg_bool("is_sum_max").unwrap_or(Some(false)).unwrap_or(false);
            let lock = op.kwarg_bool("lock").unwrap_or(Some(false)).unwrap_or(false);
            let newline = op.kwarg_bool("newline").unwrap_or(Some(false)).unwrap_or(false);
            merge_by_punctuation(
                result,
                MergeByPunctuation { punctuation, max_words, max_chars, is_sum_max, lock, newline },
            );
            Ok(())
        }
        "merge_all_segments" => {
            merge_all_segments(result);
            Ok(())
        }
        other => Err(UnsupportedMethod(other.to_string())),
    }
}

impl RegroupOp {
    /// Look up a bound kwarg by name. `None` means the kwarg was absent (so the
    /// caller should use the method default); `Some(v)` is the bound JSON value.
    fn kwarg(&self, name: &str) -> Option<&Value> {
        self.kwargs.iter().find(|(k, _)| k == name).map(|(_, v)| v)
    }

    /// Read a kwarg as its raw JSON value, cloned. Used by `split_by_duration`,
    /// which keeps `max_dur` verbatim so the history string formats it exactly
    /// as the DSL coerced it (an int `4` stays `4`, a float `4.0` stays `4.0`).
    fn kwarg_value(&self, name: &str) -> Option<Value> {
        self.kwarg(name).cloned()
    }

    /// Read a kwarg as `Option<f64>`. Outer `None` = absent; inner `None` would
    /// be an explicit JSON null (not produced by the parser, but handled).
    fn kwarg_f64(&self, name: &str) -> Option<Option<f64>> {
        self.kwarg(name).map(serde_json::Value::as_f64)
    }

    /// Read a kwarg as `Option<usize>` (a non-negative integer like `max_chars`).
    fn kwarg_usize(&self, name: &str) -> Option<Option<usize>> {
        self.kwarg(name).map(|v| v.as_u64().map(|n| n as usize))
    }

    /// Read a kwarg as `Option<bool>`. The DSL coerces flags to ints (`0`/`1`),
    /// so any non-zero integer is truthy, matching Python's `bool(int)`.
    fn kwarg_bool(&self, name: &str) -> Option<Option<bool>> {
        self.kwarg(name).map(|v| v.as_i64().map(|n| n != 0))
    }
}

/// Format an `f64` the way Python's `str()`/`f'{x}'` does for the small floats
/// the regroup history records (e.g. `2.5` -> `"2.5"`, `3.0` -> `"3.0"`).
fn py_float(v: f64) -> String {
    if v == v.trunc() && v.is_finite() {
        format!("{v:.1}")
    } else {
        format!("{v}")
    }
}

/// Format a coerced DSL numeric value for a history entry the way Python's
/// `f'{x}'` does. An integer (`sd=4`) renders as `4`; a float (`sd=4.0`) renders
/// via [`py_float`] (`4.0`). Falls back to the value's display for any non-number.
fn py_number(v: &Value) -> String {
    match v {
        Value::Number(n) if n.is_f64() => n.as_f64().map_or_else(|| n.to_string(), py_float),
        Value::Number(n) => n.to_string(),
        other => other.to_string(),
    }
}

/// Append one regroup op's encoded form to the history string, mirroring the
/// `if self._regroup_history: += '_'` join upstream uses.
fn push_history(result: &mut WhisperResult, entry: &str) {
    if !result.regroup_history.is_empty() {
        result.regroup_history.push('_');
    }
    result.regroup_history.push_str(entry);
}

/// `WhisperResult.has_words`: any segment carries word timings.
fn result_has_words(result: &WhisperResult) -> bool {
    result.segments.iter().any(Segment::has_words)
}

/// Port of `WhisperResult.clamp_max` (median-based per-segment duration clamp).
///
/// Clamps word durations above `medium_factor * median_word_duration` per
/// segment (only when the segment has >1 word), falling back to / additionally
/// bounding by `max_dur`. With `clip_start = None` only the first word's start
/// and the last word's end are clamped; otherwise every word is clamped on the
/// side `clip_start` selects.
fn clamp_max(result: &mut WhisperResult, medium_factor: Option<f64>, max_dur: Option<f64>, clip_start: Option<bool>) {
    // `not (medium_factor or max_dur)` — both falsy is a ValueError upstream;
    // here the staged op always supplies medium_factor, so we just no-op.
    let mf = medium_factor.filter(|&f| f != 0.0);
    if mf.is_none() && max_dur.filter(|&d| d != 0.0).is_none() {
        return;
    }
    if !result_has_words(result) {
        return;
    }

    for seg in &mut result.segments {
        let Some(words) = seg.words.as_mut() else { continue };

        let mut curr_max_dur: Option<f64> = None;
        if let Some(factor) = mf
            && words.len() > 1 {
                let mut durations: Vec<f64> = words.iter().map(WordTiming::duration).collect();
                // Python `durations[len//2]` (raw index, not an averaged median)
                // after an ascending sort. Quickselect partitions at that index
                // in O(n); the element it lands there is exactly what a full
                // ascending sort would place there.
                let mid = durations.len() / 2;
                let (_, median, _) =
                    durations.select_nth_unstable_by(mid, |a, b| a.partial_cmp(b).expect("finite durations"));
                curr_max_dur = Some(factor * *median);
            }
        if let Some(md) = max_dur
            && curr_max_dur.is_none_or(|c| c > md) {
                curr_max_dur = Some(md);
            }
        let Some(cap) = curr_max_dur.filter(|&c| c != 0.0) else { continue };

        match clip_start {
            None => {
                clamp_word(&mut words[0], cap, true);
                let last = words.len() - 1;
                clamp_word(&mut words[last], cap, false);
            }
            Some(cs) => {
                for w in words.iter_mut() {
                    clamp_word(w, cap, cs);
                }
            }
        }
    }

    let entry = format!(
        "cm={}+{}+{}+{}",
        py_float(medium_factor.unwrap_or(0.0)),
        max_dur.filter(|&d| d != 0.0).map_or(String::new(), py_float),
        match clip_start {
            Some(true) => "True".to_string(),
            _ => String::new(),
        },
        0, // int(verbose), verbose always False for the staged ops
    );
    push_history(result, &entry);
}

/// Port of `WordTiming.clamp_max`: shrink a word whose duration exceeds
/// `max_dur` by moving its start (`clip_start = true`) or end inward.
fn clamp_word(word: &mut WordTiming, max_dur: f64, clip_start: bool) {
    if word.duration() > max_dur {
        if clip_start {
            word.set_start(word.end() - max_dur);
        } else {
            word.set_end(word.start() + max_dur);
        }
    }
}

/// Bound parameters for [`split_by_length`], matching the Python method's
/// keyword arguments.
struct SplitByLength {
    max_chars: Option<usize>,
    max_words: Option<usize>,
    even_split: bool,
    force_len: bool,
    lock: bool,
    include_lock: bool,
    newline: bool,
}

/// Port of `WhisperResult.split_by_length`: split (or insert line breaks in)
/// any segment exceeding `max_chars`/`max_words`.
fn split_by_length(result: &mut WhisperResult, p: SplitByLength) {
    if p.force_len {
        // Upstream collapses everything into one segment first so each piece
        // gets a constant length (without recording the merge in history).
        merge_all_segments_inner(result);
    }
    split_segments(
        result,
        |seg| get_length_indices(seg, p.max_chars, p.max_words, p.even_split, p.include_lock),
        p.lock,
        p.newline,
    );

    let entry = format!(
        "sl={}+{}+{}+{}+{}+{}+{}",
        p.max_chars.map_or(String::new(), |n| n.to_string()),
        p.max_words.map_or(String::new(), |n| n.to_string()),
        i32::from(p.even_split),
        i32::from(p.force_len),
        i32::from(p.lock),
        i32::from(p.include_lock),
        i32::from(p.newline),
    );
    push_history(result, &entry);
}

/// Bound parameters for [`split_by_duration`], matching the Python method's
/// keyword arguments. `max_dur` is kept as the raw coerced JSON value so the
/// history string reproduces the DSL form (int vs float) exactly.
struct SplitByDuration {
    max_dur: Option<Value>,
    even_split: bool,
    force_len: bool,
    lock: bool,
    include_lock: bool,
    newline: bool,
}

/// Port of `WhisperResult.split_by_duration`: split (or insert line breaks in)
/// any segment whose total word duration exceeds `max_dur`.
///
/// Same shape as [`split_by_length`] — it runs the shared `split_segments`
/// driver with `get_duration_indices` as the per-segment index function, then
/// appends the `sd=...` history entry.
fn split_by_duration(result: &mut WhisperResult, p: SplitByDuration) {
    if p.force_len {
        // `merge_all_segments()` is now implemented; mirror split_by_length by
        // collapsing first so each piece gets a constant length.
        merge_all_segments_inner(result);
    }
    let max_dur = p.max_dur.as_ref().and_then(Value::as_f64);
    split_segments(
        result,
        |seg| get_duration_indices(seg, max_dur, p.even_split, p.include_lock),
        p.lock,
        p.newline,
    );

    let entry = format!(
        "sd={}+{}+{}+{}+{}+{}",
        p.max_dur.as_ref().map_or(String::new(), py_number),
        i32::from(p.even_split),
        i32::from(p.force_len),
        i32::from(p.lock),
        i32::from(p.include_lock),
        i32::from(p.newline),
    );
    push_history(result, &entry);
}

/// Port of `Segment.get_duration_indices` (stable-ts 2.17.5): the word indices
/// after which to split so each piece's total word duration stays near
/// `max_dur`.
///
/// Returns no splits when the segment is wordless, `max_dur` is absent, or the
/// segment's total duration is already within `max_dur`. With `even_split` the
/// splits are distributed evenly (the same `ceil`/`argmin` scheme
/// `get_length_indices` uses for characters); otherwise it splits greedily
/// after the first non-locked word that pushes the running duration over
/// `max_dur`.
fn get_duration_indices(seg: &Segment, max_dur: Option<f64>, even_split: bool, include_lock: bool) -> Vec<usize> {
    let Some(words) = seg.words.as_ref() else { return Vec::new() };
    let Some(max_dur) = max_dur else { return Vec::new() };
    if words.is_empty() {
        return Vec::new();
    }

    // `np.sum([w.duration ...]) <= max_dur` -> nothing to split.
    let durations: Vec<f64> = words.iter().map(WordTiming::duration).collect();
    let total_duration: f64 = durations.iter().sum();
    if total_duration <= max_dur {
        return Vec::new();
    }

    if even_split {
        // splits = ceil(total / max_dur); dur_per_split = total / splits.
        let splits = (total_duration / max_dur).ceil();
        let dur_per_split = total_duration / splits;
        // cum_dur = np.cumsum(durations[:-1]).
        let cum: Vec<f64> = cumsum_f64(&durations[..durations.len() - 1]);
        (1..splits as usize)
            .map(|i| argmin_abs(&cum, i as f64 * dur_per_split))
            .collect()
    } else {
        let locked: Vec<usize> = if include_lock { get_locked_indices(words) } else { Vec::new() };
        let mut indices = Vec::new();
        let mut curr_total_dur = 0.0;
        for (i, &dur) in durations.iter().enumerate() {
            curr_total_dur += dur;
            if i != 0 && curr_total_dur > max_dur && !locked.contains(&(i - 1)) {
                indices.push(i - 1);
                curr_total_dur = dur;
            }
        }
        indices
    }
}

/// Bound parameters for [`split_by_gap`], matching the Python method's keyword
/// arguments. `max_gap` is kept as the raw coerced JSON value so the history
/// string reproduces the DSL form (int vs float) exactly.
struct SplitByGap {
    max_gap: Option<Value>,
    lock: bool,
    newline: bool,
}

/// Port of `WhisperResult.split_by_gap`: split (or insert line breaks in) any
/// segment where the gap between two adjacent words exceeds `max_gap`.
///
/// Same shape as [`split_by_length`]/[`split_by_duration`] — it runs the shared
/// `split_segments` driver with `get_gap_indices` as the per-segment index
/// function, then appends the `sg=...` history entry.
fn split_by_gap(result: &mut WhisperResult, p: SplitByGap) {
    // Upstream default when the kwarg is absent is `0.1`; an explicit JSON null
    // (not produced by the parser) maps to `0`, matching the `max_gap is None`
    // branch in `Segment.get_gap_indices`.
    let max_gap = match p.max_gap.as_ref() {
        Some(v) => v.as_f64().unwrap_or(0.0),
        None => 0.1,
    };
    split_segments(result, |seg| get_gap_indices(seg, max_gap), p.lock, p.newline);

    // `sg={max_gap}+{int(lock)}+{int(newline)}` — the captured stable-ts records
    // exactly these three fields (golden `regroup_history`: `sg=0.5+0+0`).
    let entry = format!(
        "sg={}+{}+{}",
        p.max_gap.as_ref().map_or_else(|| py_float(0.1), py_number),
        i32::from(p.lock),
        i32::from(p.newline),
    );
    push_history(result, &entry);
}

/// Port of `Segment.get_gap_indices` (stable-ts 2.19.1): the word indices after
/// which to split where the gap between word `i`'s end and word `i+1`'s start
/// exceeds `max_gap`, excluding locked boundaries.
fn get_gap_indices(seg: &Segment, max_gap: f64) -> Vec<usize> {
    let Some(words) = seg.words.as_ref() else { return Vec::new() };
    if words.len() < 2 {
        return Vec::new();
    }
    let locked = get_locked_indices(words);
    words
        .windows(2)
        .enumerate()
        .filter_map(|(i, w)| {
            let gap = w[1].start() - w[0].end();
            (gap > max_gap && !locked.contains(&i)).then_some(i)
        })
        .collect()
}

/// Bound parameters for [`split_by_punctuation`], matching the Python method's
/// keyword arguments. `punctuation` is kept as the raw coerced JSON value so the
/// history string reproduces the DSL form exactly.
struct SplitByPunctuation {
    punctuation: Option<Value>,
    lock: bool,
    newline: bool,
    min_words: Option<usize>,
    min_chars: Option<usize>,
    min_dur: Option<f64>,
}

/// One punctuation token: either a plain string (split after a word that ends
/// with it, or before a word that starts with it) or an `(ending, beginning)`
/// pair (split between `w0` ending with `ending` and `w1` starting with
/// `beginning`), mirroring `str_to_valid_type`'s `/`/`*` coercion.
enum PunctToken {
    Plain(String),
    Pair(String, String),
}

/// Port of `WhisperResult.split_by_punctuation`: split (or insert line breaks
/// in) segments at words bordering `punctuation`, optionally gated so only
/// segments meeting `min_words`/`min_chars`/`min_dur` are touched.
fn split_by_punctuation(result: &mut WhisperResult, p: SplitByPunctuation) {
    let Some(punct_value) = p.punctuation.as_ref() else { return };
    let tokens = parse_punctuation(punct_value);

    let gated = p.min_words.is_some() || p.min_chars.is_some() || p.min_dur.is_some();
    let over_max = |seg: &Segment| -> bool {
        // `min_words and len(words) >= min_words` etc.; a `0` min is falsy in
        // Python, so it never gates (mirrored by treating `Some(0)` as off).
        (p.min_words.is_some_and(|m| m != 0 && seg_word_count(seg) >= m))
            || (p.min_chars.is_some_and(|m| m != 0 && seg_char_count(seg) >= m))
            || (p.min_dur.is_some_and(|m| m != 0.0 && (seg.end() - seg.start()) >= m))
    };

    split_segments(
        result,
        |seg| {
            if gated && !over_max(seg) {
                Vec::new()
            } else {
                get_punctuation_indices(seg, &tokens)
            }
        },
        p.lock,
        p.newline,
    );

    // `sp={punct_str}+{int(lock)}+{int(newline)}` — the captured stable-ts
    // records exactly these three fields (golden `regroup_history`:
    // `sp=,* /，+0+0`); the `min_words`/`min_chars`/`min_dur` gates affect which
    // segments split but are not part of this build's history encoding.
    let entry = format!("sp={}+{}+{}", punct_str(&tokens), i32::from(p.lock), i32::from(p.newline));
    push_history(result, &entry);
}

/// Coerce a parsed `punctuation` JSON value into the token list. A bare string
/// becomes a single plain token (`isinstance(punctuation, str)` upstream); a
/// list yields one token per element, each a plain string or a two-element
/// `[ending, beginning]` pair from the `*`-split coercion.
fn parse_punctuation(value: &Value) -> Vec<PunctToken> {
    match value {
        Value::String(s) => vec![PunctToken::Plain(s.clone())],
        Value::Array(items) => items
            .iter()
            .map(|item| match item {
                Value::Array(pair) if pair.len() == 2 => PunctToken::Pair(
                    pair[0].as_str().unwrap_or_default().to_string(),
                    pair[1].as_str().unwrap_or_default().to_string(),
                ),
                other => PunctToken::Plain(other.as_str().unwrap_or_default().to_string()),
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// Port of `Segment.get_punctuation_indices` (stable-ts 2.19.1): the word
/// indices after which to split given the punctuation tokens, excluding locked
/// boundaries.
fn get_punctuation_indices(seg: &Segment, tokens: &[PunctToken]) -> Vec<usize> {
    let Some(words) = seg.words.as_ref() else { return Vec::new() };
    if words.len() < 2 {
        return Vec::new();
    }
    let last = words.len() - 1;
    let mut indices: Vec<usize> = Vec::new();
    for token in tokens {
        match token {
            PunctToken::Plain(p) => {
                // `for i, s in enumerate(self.words[:-1])`.
                for (i, w) in words[..last].iter().enumerate() {
                    if !p.is_empty() && w.word.ends_with(p.as_str()) {
                        indices.push(i);
                    } else if i != 0 && !p.is_empty() && w.word.starts_with(p.as_str()) {
                        indices.push(i - 1);
                    }
                }
            }
            PunctToken::Pair(ending, beginning) => {
                // `zip(words[:-1], words[1:])` -> boundary i after word i.
                for (i, pair) in words.windows(2).enumerate() {
                    if (ending.is_empty() || pair[0].word.ends_with(ending.as_str()))
                        && (beginning.is_empty() || pair[1].word.starts_with(beginning.as_str()))
                    {
                        indices.push(i);
                    }
                }
            }
        }
    }
    // `sorted(set(indices) - set(get_locked_indices()))`.
    let locked = get_locked_indices(words);
    indices.retain(|i| !locked.contains(i));
    indices.sort_unstable();
    indices.dedup();
    indices
}

/// `len(x.words)` for the `min_words` gate (a word-bearing segment).
fn seg_word_count(seg: &Segment) -> usize {
    seg.words.as_ref().map_or(0, Vec::len)
}

/// `Segment.char_count()` for word-bearing segments: `sum(len(w.word))`
/// (code-point count), else the text length.
fn seg_char_count(seg: &Segment) -> usize {
    match seg.words.as_ref() {
        Some(w) if !w.is_empty() => w.iter().map(|x| x.word.chars().count()).sum(),
        _ => seg.text().chars().count(),
    }
}

/// Render a parsed punctuation token list back to the history string form
/// (`'/'.join(p if str else '*'.join(p))`).
fn punct_str(tokens: &[PunctToken]) -> String {
    tokens
        .iter()
        .map(|t| match t {
            PunctToken::Plain(s) => s.clone(),
            PunctToken::Pair(a, b) => format!("{a}*{b}"),
        })
        .collect::<Vec<_>>()
        .join("/")
}

/// Bound parameters for [`merge_by_gap`], matching the Python method's keyword
/// arguments. `min_gap` is kept as the raw coerced JSON value so the history
/// string reproduces the DSL form (int vs float) exactly.
struct MergeByGap {
    min_gap: Option<Value>,
    max_words: Option<usize>,
    max_chars: Option<usize>,
    is_sum_max: bool,
    lock: bool,
    newline: bool,
}

/// Port of `WhisperResult.merge_by_gap`: merge a segment into the next when the
/// gap between them is `<= min_gap`, subject to the `max_words`/`max_chars` cap.
///
/// Computes the merge-candidate boundary indices with `get_merge_gap_indices`
/// (the result-level `get_gap_indices` for merging), runs the shared
/// `merge_segments` driver to fuse them, then appends the `mg=...` history entry.
fn merge_by_gap(result: &mut WhisperResult, p: MergeByGap) {
    // Upstream default when the kwarg is absent is `0.1`; an explicit JSON null
    // (not produced by the parser) maps to `0`, matching the `min_gap is None`
    // branch in `WhisperResult.get_gap_indices`.
    let min_gap = match p.min_gap.as_ref() {
        Some(v) => v.as_f64().unwrap_or(0.0),
        None => 0.1,
    };
    let indices = get_merge_gap_indices(result, min_gap);
    merge_segments(
        result,
        &indices,
        MergeCaps { max_words: p.max_words, max_chars: p.max_chars, is_sum_max: p.is_sum_max },
        p.lock,
        p.newline,
    );

    // `mg={min_gap}+{max_words or ""}+{max_chars or ""}+{int(is_sum_max)}+
    // {int(lock)}+{int(newline)}` — the captured stable-ts records exactly these
    // six fields (golden `regroup_history`: `mg=0.3+3++0+0+0`). A falsy (None/0)
    // `max_words`/`max_chars` renders empty, matching Python's `x or ""`.
    let entry = format!(
        "mg={}+{}+{}+{}+{}+{}",
        p.min_gap.as_ref().map_or_else(|| py_float(0.1), py_number),
        max_cap_str(p.max_words),
        max_cap_str(p.max_chars),
        i32::from(p.is_sum_max),
        i32::from(p.lock),
        i32::from(p.newline),
    );
    push_history(result, &entry);
}

/// Bound parameters for [`merge_by_punctuation`], matching the Python method's
/// keyword arguments. `punctuation` is kept as the raw coerced JSON value so the
/// history string reproduces the DSL form exactly.
struct MergeByPunctuation {
    punctuation: Option<Value>,
    max_words: Option<usize>,
    max_chars: Option<usize>,
    is_sum_max: bool,
    lock: bool,
    newline: bool,
}

/// Port of `WhisperResult.merge_by_punctuation`: merge across a segment boundary
/// when the earlier segment ends with (or the later begins with) one of the
/// `punctuation` tokens, subject to the `max_words`/`max_chars` cap.
///
/// Computes the merge-candidate boundary indices with
/// `get_merge_punctuation_indices` (the result-level `get_punctuation_indices`
/// for merging), runs the shared `merge_segments` driver, then appends the
/// `mp=...` history entry.
fn merge_by_punctuation(result: &mut WhisperResult, p: MergeByPunctuation) {
    let Some(punct_value) = p.punctuation.as_ref() else { return };
    let tokens = parse_punctuation(punct_value);

    let indices = get_merge_punctuation_indices(result, &tokens);
    merge_segments(
        result,
        &indices,
        MergeCaps { max_words: p.max_words, max_chars: p.max_chars, is_sum_max: p.is_sum_max },
        p.lock,
        p.newline,
    );

    // `mp={punct_str}+{max_words or ""}+{max_chars or ""}+{int(is_sum_max)}+
    // {int(lock)}+{int(newline)}` — the captured stable-ts records exactly these
    // six fields (golden `regroup_history`: `mp=.* /。/?/？+++0+0+0`).
    let entry = format!(
        "mp={}+{}+{}+{}+{}+{}",
        punct_str(&tokens),
        max_cap_str(p.max_words),
        max_cap_str(p.max_chars),
        i32::from(p.is_sum_max),
        i32::from(p.lock),
        i32::from(p.newline),
    );
    push_history(result, &entry);
}

/// Render an optional `max_words`/`max_chars` cap for a merge history entry the
/// way Python's `f'{x or ""}'` does: a present, non-zero cap renders its int; a
/// `None` (or falsy `0`) renders empty.
fn max_cap_str(cap: Option<usize>) -> String {
    cap.filter(|&n| n != 0).map_or(String::new(), |n| n.to_string())
}

/// The `max_words`/`max_chars`/`is_sum_max` cap that gates each merge, mirroring
/// the corresponding kwargs of `WhisperResult._merge_segments`.
struct MergeCaps {
    max_words: Option<usize>,
    max_chars: Option<usize>,
    is_sum_max: bool,
}

/// Port of `WhisperResult.get_locked_indices` (the result-level overload used
/// for merging): boundary `i` is locked when segment `i+1`'s left edge or
/// segment `i`'s right edge is locked.
fn get_segment_locked_indices(result: &WhisperResult) -> Vec<usize> {
    // Python zips segments[1:] with segments[:-1]; index i covers the boundary
    // after segment i (between segment i and segment i+1).
    if result.segments.len() < 2 {
        return Vec::new();
    }
    (0..result.segments.len() - 1)
        .filter(|&i| result.segments[i + 1].left_locked() || result.segments[i].right_locked())
        .collect()
}

/// Port of `WhisperResult.get_gap_indices` (for merging): boundary indices where
/// the gap between segment `i`'s end and segment `i+1`'s start is `<= min_gap`,
/// excluding locked boundaries.
fn get_merge_gap_indices(result: &WhisperResult, min_gap: f64) -> Vec<usize> {
    if result.segments.len() < 2 {
        return Vec::new();
    }
    let locked = get_segment_locked_indices(result);
    let mut indices: Vec<usize> = (0..result.segments.len() - 1)
        .filter(|&i| {
            let gap = result.segments[i + 1].start() - result.segments[i].end();
            gap <= min_gap && !locked.contains(&i)
        })
        .collect();
    indices.sort_unstable();
    indices.dedup();
    indices
}

/// Port of `WhisperResult.get_punctuation_indices` (for merging): boundary
/// indices where the earlier segment's text ends with (or, for a plain token,
/// the later begins with) a punctuation token, excluding locked boundaries.
fn get_merge_punctuation_indices(result: &WhisperResult, tokens: &[PunctToken]) -> Vec<usize> {
    if result.segments.len() < 2 {
        return Vec::new();
    }
    let last = result.segments.len() - 1;
    let mut indices: Vec<usize> = Vec::new();
    for token in tokens {
        match token {
            PunctToken::Plain(p) => {
                // `for i, s in enumerate(self.segments[:-1])`.
                for i in 0..last {
                    let text = result.segments[i].text();
                    if !p.is_empty() && text.ends_with(p.as_str()) {
                        indices.push(i);
                    } else if i != 0 && !p.is_empty() && text.starts_with(p.as_str()) {
                        indices.push(i - 1);
                    }
                }
            }
            PunctToken::Pair(ending, beginning) => {
                // `zip(segments[:-1], segments[1:])` -> boundary i after segment i.
                for i in 0..last {
                    let s0 = result.segments[i].text();
                    let s1 = result.segments[i + 1].text();
                    if (ending.is_empty() || s0.ends_with(ending.as_str()))
                        && (beginning.is_empty() || s1.starts_with(beginning.as_str()))
                    {
                        indices.push(i);
                    }
                }
            }
        }
    }
    // `sorted(set(indices) - set(get_locked_indices()))`.
    let locked = get_segment_locked_indices(result);
    indices.retain(|i| !locked.contains(i));
    indices.sort_unstable();
    indices.dedup();
    indices
}

/// Port of `WhisperResult._merge_segments`: for each candidate boundary index
/// (in reverse order), fuse segment `i` into segment `i+1` unless the
/// `max_words`/`max_chars` cap forbids it, then drop any now-wordless segments.
fn merge_segments(result: &mut WhisperResult, indices: &[usize], caps: MergeCaps, lock: bool, newline: bool) {
    if indices.is_empty() {
        return;
    }
    for &i in indices.iter().rev() {
        if merge_capped(&result.segments[i], &result.segments[i + 1], &caps) {
            continue;
        }
        let merged = merge_two_segments(&result.segments[i], &result.segments[i + 1], lock, newline);
        result.segments[i] = merged;
        result.segments.remove(i + 1);
    }
    remove_no_word_segments(result);
}

/// Port of the `_merge_segments` skip guard: true when the `max_words`/
/// `max_chars` cap forbids merging segments `seg` and `next_seg`.
///
/// With `is_sum_max` the cap applies to the merged segment (the sum of both
/// counts); otherwise it forbids the merge only when *both* segments already
/// exceed the cap. A falsy (`None`/`0`) cap never gates (Python `max_words and
/// ...`). The `max_words` branch additionally requires `seg.has_words` upstream.
fn merge_capped(seg: &Segment, next_seg: &Segment, caps: &MergeCaps) -> bool {
    let words_blocks = caps.max_words.is_some_and(|m| m != 0) && seg.has_words() && {
        let a = seg_word_count(seg);
        let b = seg_word_count(next_seg);
        let m = caps.max_words.expect("checked");
        if caps.is_sum_max {
            a + b > m
        } else {
            a > m && b > m
        }
    };
    let chars_blocks = caps.max_chars.is_some_and(|m| m != 0) && {
        let a = seg_char_count(seg);
        let b = seg_char_count(next_seg);
        let m = caps.max_chars.expect("checked");
        if caps.is_sum_max {
            a + b > m
        } else {
            a > m && b > m
        }
    };
    words_blocks || chars_blocks
}

/// Port of `Segment.add` plus the `add_segments` lock handling: fuse two
/// adjacent word-bearing segments by concatenating their word lists (so the
/// merged `start`/`end`/`text` derive from the words), cloning the first
/// segment's per-segment metadata. With `newline`, append `\n` to the boundary
/// word; with `lock`, lock the right edge of the first segment's last word and
/// the left edge of the second segment's first word across the seam.
fn merge_two_segments(seg: &Segment, next_seg: &Segment, lock: bool, newline: bool) -> Segment {
    let mut merged = seg.clone();
    if seg.ori_has_words() && next_seg.ori_has_words() {
        let mut words: Vec<WordTiming> = seg.words.as_ref().map_or_else(Vec::new, std::clone::Clone::clone);
        let boundary = words.len();
        if let Some(next_words) = next_seg.words.as_ref() {
            words.extend(next_words.iter().cloned());
        }
        if newline {
            // `if not words[len(self.words)-1].word.endswith('\n')`.
            if boundary > 0 && !words[boundary - 1].word.ends_with('\n') {
                words[boundary - 1].word.push('\n');
            }
        }
        if lock && boundary > 0 {
            words[boundary - 1].lock_right();
            if boundary < words.len() {
                words[boundary].lock_left();
            }
        }
        merged.set_words(words);
    } else {
        // Wordless seam: concatenate text and extend the end to the next
        // segment's end (start stays the first's). `add_segments`' lock branch
        // only fires for word-bearing segments, so nothing to lock here.
        let first_text = seg.text();
        let mut text = first_text.clone();
        text.push_str(&next_seg.text());
        if newline && !first_text.is_empty() && !first_text.ends_with('\n') {
            // `if self_copy.text[len(self.text)-1] != '\n'`: insert a break at
            // the seam between the two segments' text (after the first's text).
            text.insert(first_text.len(), '\n');
        }
        let start = merged.start();
        let end = next_seg.end();
        merged.set_default_text(text);
        merged.set_default_span(start, end);
    }
    merged
}

/// Port of `WhisperResult.merge_all_segments`: collapse every segment into one.
///
/// Concatenates all words (in order) into a single segment cloned from the
/// first (so its per-segment metadata carries over), recomputing `start`/`end`/
/// `text` from the merged words, then appends the `ms` history entry. The
/// wordless fallback merges the segments' text/tokens onto the first segment's
/// defaults, matching the upstream `else` branch.
fn merge_all_segments(result: &mut WhisperResult) {
    if result.segments.is_empty() {
        return;
    }
    merge_all_segments_inner(result);
    push_history(result, "ms");
}

/// The history-free body of [`merge_all_segments`], shared with the
/// `split_by_duration(force_len=True)` pre-merge (`split_by_length` and
/// `split_by_duration` both call `merge_all_segments()` without recording).
fn merge_all_segments_inner(result: &mut WhisperResult) {
    if result.segments.is_empty() {
        return;
    }
    let mut merged = result.segments[0].clone();
    if result_has_words(result) {
        // `all_words` = chain of every segment's words, in order.
        let all_words: Vec<WordTiming> = result
            .segments
            .iter()
            .filter_map(|s| s.words.as_ref())
            .flatten()
            .cloned()
            .collect();
        merged.set_words(all_words);
    } else {
        // Wordless: text is the concatenation of every segment's text, and the
        // end extends to the last segment's end (start stays the first's).
        let text: String = result.segments.iter().map(Segment::text).collect();
        let end = result.segments[result.segments.len() - 1].end();
        let start = merged.start();
        merged.set_default_text(text);
        merged.set_default_span(start, end);
    }
    result.segments = vec![merged];
}

/// Port of `Segment.get_locked_indices`: positions where word `i` and `i+1`
/// must stay together (either side locked across the boundary).
fn get_locked_indices(words: &[WordTiming]) -> Vec<usize> {
    // Python zips words[1:] with words[:-1]; index i covers the boundary after
    // word i (between word i and word i+1).
    words
        .windows(2)
        .enumerate()
        .filter_map(|(i, w)| (w[1].left_locked || w[0].right_locked).then_some(i))
        .collect()
}

/// Port of `Segment.get_length_indices`: the word indices after which to split
/// the segment so each piece stays within `max_chars`/`max_words`.
fn get_length_indices(
    seg: &Segment,
    max_chars: Option<usize>,
    max_words: Option<usize>,
    even_split: bool,
    include_lock: bool,
) -> Vec<usize> {
    let Some(words) = seg.words.as_ref() else { return Vec::new() };
    if words.is_empty() || (max_chars.is_none() && max_words.is_none()) {
        return Vec::new();
    }
    assert!(
        max_chars != Some(0) && max_words != Some(0),
        "max_chars and max_words must be greater 0, but got {max_chars:?} and {max_words:?}"
    );
    if words.len() < 2 {
        return Vec::new();
    }

    // Per-word character length is Python `len(word.word)` = code-point count.
    let char_lens: Vec<usize> = words.iter().map(|w| w.word.chars().count()).collect();

    if even_split {
        even_length_indices(words.len(), &char_lens, max_chars, max_words)
    } else {
        uneven_length_indices(words, &char_lens, max_chars, max_words, include_lock)
    }
}

/// The `even_split = True` branch of `get_length_indices`.
fn even_length_indices(
    n_words: usize,
    char_lens: &[usize],
    max_chars: Option<usize>,
    max_words: Option<usize>,
) -> Vec<usize> {
    let char_count: usize = max_chars.map_or(0, |_| char_lens.iter().sum());
    let word_count = n_words;

    let mut indices: Vec<usize> = Vec::new();
    let mut exceed_words = max_words.is_some_and(|m| word_count > m);

    if let Some(mc) = max_chars
        && char_count > mc {
            // splits = ceil(char_count / max_chars).
            let splits = char_count.div_ceil(mc);
            let chars_per_split = char_count as f64 / splits as f64;
            // cum_char_count over words[:-1].
            let cum: Vec<f64> = prefix_sums_f64(&char_lens[..n_words - 1]);
            indices = (1..splits)
                .map(|i| argmin_abs(&cum, i as f64 * chars_per_split))
                .collect();
            if let Some(mw) = max_words {
                // exceed_words = any piece longer than max_words words.
                let bounds: Vec<usize> = std::iter::once(0)
                    .chain(indices.iter().copied())
                    .collect();
                let ends: Vec<usize> = indices.iter().copied().chain(std::iter::once(n_words)).collect();
                exceed_words = bounds.iter().zip(&ends).any(|(&i, &j)| j - i + 1 > mw);
            }
        }

    if exceed_words {
        let mw = max_words.expect("exceed_words implies max_words set");
        let splits = word_count.div_ceil(mw);
        let words_per_split = word_count as f64 / splits as f64;
        // cum_word_count = 1..=n_words.
        let cum: Vec<f64> = (1..=n_words).map(|x| x as f64).collect();
        indices = (1..splits)
            .map(|i| argmin_abs(&cum, i as f64 * words_per_split))
            .collect();
    }

    indices
}

/// The `even_split = False` branch of `get_length_indices`.
fn uneven_length_indices(
    words: &[WordTiming],
    char_lens: &[usize],
    max_chars: Option<usize>,
    max_words: Option<usize>,
    include_lock: bool,
) -> Vec<usize> {
    let locked: Vec<usize> = if include_lock { get_locked_indices(words) } else { Vec::new() };
    let mut indices = Vec::new();
    let mut curr_words = 0usize;
    let mut curr_chars = 0usize;
    for (i, &clen) in char_lens.iter().enumerate() {
        curr_words += 1;
        curr_chars += clen;
        if i != 0 {
            let over_chars = max_chars.is_some_and(|m| curr_chars > m);
            let over_words = max_words.is_some_and(|m| curr_words > m);
            if (over_chars || over_words) && !locked.contains(&(i - 1)) {
                indices.push(i - 1);
                curr_words = 1;
                curr_chars = clen;
            }
        }
    }
    indices
}

/// Port of `WhisperResult._split_segments`: for each segment (in reverse order)
/// compute the split indices, then either insert `\n` (`newline`) at those word
/// boundaries or replace the segment with the sub-segments `Segment.split`
/// produces.
fn split_segments<F>(result: &mut WhisperResult, get_indices: F, lock: bool, newline: bool)
where
    F: Fn(&Segment) -> Vec<usize>,
{
    for i in (0..result.segments.len()).rev() {
        let mut indices = get_indices(&result.segments[i]);
        indices.sort_unstable();
        indices.dedup();
        if indices.is_empty() {
            continue;
        }

        if newline {
            apply_newline(&mut result.segments[i], &mut indices, lock);
        } else {
            let new_segments = split_segment(&result.segments[i], indices, lock);
            result.segments.splice(i..=i, new_segments);
        }
    }
    remove_no_word_segments(result);
}

/// The `newline` branch of `_split_segments`: append `\n` to the word at each
/// split index (skipping the final-word index and any word that already ends in
/// a newline), optionally locking across each break.
fn apply_newline(seg: &mut Segment, indices: &mut Vec<usize>, lock: bool) {
    let Some(words) = seg.words.as_mut() else { return };
    let last_idx = words.len() - 1;
    // Drop a trailing split that lands on the very last word (no break needed).
    if indices.last() == Some(&last_idx) {
        indices.pop();
    }
    for &word_idx in indices.iter() {
        if words[word_idx].word.ends_with('\n') {
            continue;
        }
        words[word_idx].word.push('\n');
        if lock {
            words[word_idx].lock_right();
            if word_idx + 1 < words.len() {
                words[word_idx + 1].lock_left();
            }
        }
    }
}

/// Port of `Segment.split`: cut the segment's words at each split index into
/// new word-bearing segments (cloning the parent's per-segment metadata).
fn split_segment(seg: &Segment, mut indices: Vec<usize>, lock: bool) -> Vec<Segment> {
    let words = seg.words.as_ref().expect("split only runs on word-bearing segments");
    if indices.is_empty() {
        return Vec::new();
    }
    // Ensure the final word terminates the last piece.
    if indices.last() != Some(&(words.len() - 1)) {
        indices.push(words.len() - 1);
    }

    // Clone the parent's per-segment metadata once (without its word vector),
    // then hand each piece its own disjoint word slice. This clones each word
    // exactly once across all pieces instead of cloning the full word vector
    // once per piece.
    let mut template = seg.clone();
    let words = template.words.take().expect("checked above");

    let mut new_segments = Vec::with_capacity(indices.len());
    let mut prev_i = 0usize;
    for idx in indices {
        let end = idx + 1;
        let mut piece = template.clone();
        piece.words = Some(words[prev_i..end].to_vec());
        new_segments.push(piece);
        prev_i = end;
    }

    if lock {
        let n = new_segments.len();
        for (k, s) in new_segments.iter_mut().enumerate() {
            if k == 0 {
                s.lock_right();
            } else if k == n - 1 {
                s.lock_left();
            } else {
                s.lock_both();
            }
        }
    }
    new_segments
}

/// Port of `WhisperResult.remove_no_word_segments`: drop any segment that
/// originally had words but now has none. (`reassign_ids` is a no-op for parity
/// since ids are not serialized.)
fn remove_no_word_segments(result: &mut WhisperResult) {
    result.segments.retain(|s| !s.ori_has_words() || s.has_words());
}

/// Prefix character counts as `f64`, mirroring `np.cumsum`.
fn prefix_sums_f64(lens: &[usize]) -> Vec<f64> {
    let mut acc = 0.0;
    lens.iter()
        .map(|&l| {
            acc += l as f64;
            acc
        })
        .collect()
}

/// Prefix sums of `f64` values, mirroring `np.cumsum` over word durations.
fn cumsum_f64(values: &[f64]) -> Vec<f64> {
    let mut acc = 0.0;
    values
        .iter()
        .map(|&v| {
            acc += v;
            acc
        })
        .collect()
}

/// `np.abs(arr - target).argmin()`: index of the element closest to `target`,
/// ties going to the lowest index (numpy `argmin` returns the first minimum).
fn argmin_abs(arr: &[f64], target: f64) -> usize {
    let mut best = 0usize;
    let mut best_dist = (arr[0] - target).abs();
    for (i, &v) in arr.iter().enumerate().skip(1) {
        let d = (v - target).abs();
        if d < best_dist {
            best = i;
            best_dist = d;
        }
    }
    best
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

    #[test]
    fn argmin_abs_breaks_ties_to_lowest_index() {
        // numpy argmin returns the first of equal minima.
        assert_eq!(argmin_abs(&[0.0, 2.0, 2.0, 5.0], 2.0), 1);
        assert_eq!(argmin_abs(&[1.0, 3.0], 2.0), 0);
        assert_eq!(argmin_abs(&[4.0], 100.0), 0);
    }

    #[test]
    fn py_float_matches_python_str() {
        assert_eq!(py_float(2.5), "2.5");
        assert_eq!(py_float(3.0), "3.0");
        assert_eq!(py_float(0.3), "0.3");
    }

    /// Helper: a segment carrying only the word timings clamp_max touches.
    fn seg_with_words(words: Vec<(f64, f64)>) -> Value {
        let words: Vec<Value> = words
            .into_iter()
            .map(|(start, end)| json!({"word": "x", "start": start, "end": end, "probability": 0.5}))
            .collect();
        json!({"words": words})
    }

    /// Falsifier for the two `clamp_max` parity defects against stable-ts
    /// 2.19.1 (`result.py` `clamp_max`): the median index is `len//2` (not
    /// `len//2 + 1`) and the per-segment gate is `len(words) > 1` (not `> 2`).
    ///
    /// The driving case is a 3-word segment whose word durations are
    /// `[0.2, 0.2, 0.9]`. Sorted ascending that is `[0.2, 0.2, 0.9]`; the
    /// 2.19.1 cap is `2.5 * durations[3//2]` = `2.5 * 0.2` = `0.5`, so under
    /// `clip_start=None` the last word's end clamps to `start + 0.5`
    /// (`0.4 + 0.5 = 0.9`). The pre-fix Rust used `durations[3//2 + 1]`
    /// = `0.9`, giving cap `2.25`, which never clamps the `0.9`-second word —
    /// so this assertion FAILS before the index fix and PASSES after it.
    ///
    /// The 2-word segment exercises the corrected `> 1` gate: 2.19.1 runs the
    /// median branch for it (the pre-fix `> 2` gate skipped it). Its timings
    /// cannot diverge — for any 2-word segment the cap is `2.5 * max(d0, d1)`,
    /// which both words sit below — so it is included to confirm the now-active
    /// branch handles a 2-word segment without panicking or altering it.
    #[test]
    fn clamp_max_median_index_and_word_gate() {
        let raw = json!({
            "segments": [
                seg_with_words(vec![(0.0, 0.2), (0.2, 0.4), (0.4, 1.3)]),
                seg_with_words(vec![(2.0, 2.1), (2.1, 3.0)]),
            ]
        });
        let mut result = WhisperResult::from_value(&raw);

        // medium_factor=2.5, max_dur=None, clip_start=None — the `cm` defaults.
        clamp_max(&mut result, Some(2.5), None, None);

        let three = result.segments[0].words.as_ref().unwrap();
        // Last word end clamped to start + (2.5 * median 0.2) = 0.4 + 0.5.
        assert_eq!(three[2].end(), 0.9, "3-word last-word end must clamp under the n//2 cap");
        // First word's duration (0.2) is below the cap, so its start is unchanged.
        assert_eq!(three[0].start(), 0.0);

        // 2-word segment: gate `> 1` now runs its median branch, but the cap
        // (2.5 * 0.9 = 2.25) exceeds both word durations, so nothing changes.
        let two = result.segments[1].words.as_ref().unwrap();
        assert_eq!(two[0].start(), 2.0);
        assert_eq!(two[1].end(), 3.0);
    }

    /// `merge_all_segments` folds every word-bearing segment into one whose
    /// `start`/`end`/`text` derive from the concatenated words, and records `ms`.
    #[test]
    fn merge_all_segments_folds_words_into_one() {
        let raw = json!({
            "regroup_history": "cm",
            "segments": [
                {"words": [
                    {"word": " Hello", "start": 0.0, "end": 0.5, "probability": 0.9},
                    {"word": " world", "start": 0.5, "end": 1.0, "probability": 0.9},
                ]},
                {"words": [
                    {"word": " again", "start": 2.0, "end": 2.5, "probability": 0.9},
                ]},
            ]
        });
        let mut result = WhisperResult::from_value(&raw);
        merge_all_segments(&mut result);

        assert_eq!(result.segments.len(), 1);
        let seg = &result.segments[0];
        assert_eq!(seg.start(), 0.0);
        assert_eq!(seg.end(), 2.5);
        assert_eq!(seg.text(), " Hello world again");
        // History append matches upstream (`_` join onto the prior `cm`).
        assert_eq!(result.regroup_history, "cm_ms");
    }

    /// `get_duration_indices(even_split=True)` splits a long segment evenly,
    /// mirroring the char-based even split but over word durations.
    #[test]
    fn duration_indices_even_split() {
        // Four 1.0s words, total 4.0s, max_dur 2.0 -> splits = 2, one cut.
        let raw = json!({"segments": [seg_with_words(vec![
            (0.0, 1.0), (1.0, 2.0), (2.0, 3.0), (3.0, 4.0),
        ])]});
        let result = WhisperResult::from_value(&raw);
        let seg = &result.segments[0];

        let indices = get_duration_indices(seg, Some(2.0), true, false);
        // dur_per_split = 2.0; cum_dur over words[:-1] = [1,2,3]; closest to 2.0
        // is index 1, so split after word 1.
        assert_eq!(indices, vec![1]);

        // Within max_dur -> no split.
        assert_eq!(get_duration_indices(seg, Some(10.0), true, false), Vec::<usize>::new());
        // Absent max_dur -> no split.
        assert_eq!(get_duration_indices(seg, None, true, false), Vec::<usize>::new());
    }

    /// `get_gap_indices` splits where the inter-word gap exceeds `max_gap`.
    #[test]
    fn gap_indices_split_on_large_gaps() {
        // Gaps between consecutive words: 0.0 (w0->w1), 0.5 (w1->w2), 0.0 (w2->w3).
        let raw = json!({"segments": [seg_with_words(vec![
            (0.0, 1.0), (1.0, 2.0), (2.5, 3.0), (3.0, 4.0),
        ])]});
        let result = WhisperResult::from_value(&raw);
        let seg = &result.segments[0];

        // max_gap 0.1 -> only the 0.5 gap after word 1 exceeds it.
        assert_eq!(get_gap_indices(seg, 0.1), vec![1]);
        // max_gap 1.0 -> no gap exceeds it.
        assert_eq!(get_gap_indices(seg, 1.0), Vec::<usize>::new());
    }

    /// `get_punctuation_indices` splits after words ending with a plain token and
    /// after the predecessor of a word starting with one; pair tokens match the
    /// `(ending, beginning)` boundary form.
    #[test]
    fn punctuation_indices_plain_and_pair() {
        let words: Vec<Value> = ["A.", " b", " C.", " d"]
            .iter()
            .enumerate()
            .map(|(i, w)| json!({"word": w, "start": i as f64, "end": i as f64 + 0.5, "probability": 0.5}))
            .collect();
        let result = WhisperResult::from_value(&json!({"segments": [{"words": words}]}));
        let seg = &result.segments[0];

        // Plain "." -> words[:-1] ending in "." are index 0 ("A.") and 2 ("C.");
        // index 3 is excluded since it's the last word (words[:-1]).
        let plain = vec![PunctToken::Plain(".".to_string())];
        assert_eq!(get_punctuation_indices(seg, &plain), vec![0, 2]);

        // Pair (".", " ") -> w0 ends "." and w1 starts " ": boundary 0 (A.|" b")
        // and boundary 2 (C.|" d").
        let pair = vec![PunctToken::Pair(".".to_string(), " ".to_string())];
        assert_eq!(get_punctuation_indices(seg, &pair), vec![0, 2]);
    }

    /// `parse_punctuation` mirrors the `str_to_valid_type` coercions: a bare
    /// string is one plain token; a list yields plain/`*`-pair tokens.
    #[test]
    fn parse_punctuation_coercions() {
        let toks = parse_punctuation(&json!([[",", " "], "，"]));
        assert_eq!(punct_str(&toks), ",* /，");
        let toks = parse_punctuation(&json!("."));
        assert_eq!(punct_str(&toks), ".");
    }

    #[test]
    fn unsupported_method_is_rejected_by_apply() {
        let mut result = WhisperResult::from_value(&json!({"segments": []}));
        let op = RegroupOp { method: "pad".to_string(), kwargs: Vec::new() };
        assert_eq!(
            apply_regroup_op(&mut result, &op).unwrap_err(),
            UnsupportedMethod("pad".to_string())
        );
    }

    /// `merge_by_gap` fuses adjacent segments whose inter-segment gap is within
    /// `min_gap`, respecting the per-segment `max_words` ceiling, and records the
    /// `mg=...` history. Two 2-word segments touch (gap 0.0) with `max_words=3`:
    /// neither exceeds 3, so they merge into one 4-word segment.
    #[test]
    fn merge_by_gap_fuses_close_segments() {
        let raw = json!({
            "segments": [
                {"words": [
                    {"word": " a", "start": 0.0, "end": 0.5, "probability": 0.9},
                    {"word": " b", "start": 0.5, "end": 1.0, "probability": 0.9},
                ]},
                {"words": [
                    {"word": " c", "start": 1.0, "end": 1.5, "probability": 0.9},
                    {"word": " d", "start": 1.5, "end": 2.0, "probability": 0.9},
                ]},
            ]
        });
        let mut result = WhisperResult::from_value(&raw);
        merge_by_gap(
            &mut result,
            MergeByGap {
                min_gap: Some(json!(0.3)),
                max_words: Some(3),
                max_chars: None,
                is_sum_max: false,
                lock: false,
                newline: false,
            },
        );

        assert_eq!(result.segments.len(), 1);
        assert_eq!(result.segments[0].text(), " a b c d");
        assert_eq!(result.regroup_history, "mg=0.3+3++0+0+0");
    }

    /// The `max_words` ceiling blocks a merge when *both* neighbours exceed it
    /// (with `is_sum_max=False`), mirroring the upstream `_merge_segments` guard.
    #[test]
    fn merge_by_gap_respects_max_words_ceiling() {
        let words = |labels: &[&str], offset: f64| -> Value {
            let ws: Vec<Value> = labels
                .iter()
                .enumerate()
                .map(|(i, w)| {
                    let s = offset + i as f64;
                    json!({"word": w, "start": s, "end": s + 0.5, "probability": 0.9})
                })
                .collect();
            json!({"words": ws})
        };
        let raw = json!({"segments": [
            words(&[" a", " b", " c", " d"], 0.0),
            words(&[" e", " f", " g", " h"], 4.0),
        ]});
        let mut result = WhisperResult::from_value(&raw);
        merge_by_gap(
            &mut result,
            MergeByGap {
                min_gap: Some(json!(0.3)),
                max_words: Some(3),
                max_chars: None,
                is_sum_max: false,
                lock: false,
                newline: false,
            },
        );
        // Both segments have 4 words (> 3), so the merge is skipped.
        assert_eq!(result.segments.len(), 2);
    }

    /// `merge_by_punctuation` fuses across a boundary where the earlier segment
    /// ends with a punctuation token and records the `mp=...` history.
    #[test]
    fn merge_by_punctuation_fuses_on_boundary() {
        let raw = json!({
            "segments": [
                {"words": [
                    {"word": " Hello.", "start": 0.0, "end": 0.5, "probability": 0.9},
                ]},
                {"words": [
                    {"word": " World", "start": 0.5, "end": 1.0, "probability": 0.9},
                ]},
            ]
        });
        let mut result = WhisperResult::from_value(&raw);
        merge_by_punctuation(
            &mut result,
            MergeByPunctuation {
                punctuation: Some(json!(".")),
                max_words: None,
                max_chars: None,
                is_sum_max: false,
                lock: false,
                newline: false,
            },
        );

        assert_eq!(result.segments.len(), 1);
        assert_eq!(result.segments[0].text(), " Hello. World");
        assert_eq!(result.regroup_history, "mp=.+++0+0+0");
    }
}
