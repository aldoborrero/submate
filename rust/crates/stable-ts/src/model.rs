//! Port of the stable-ts data model: [`WordTiming`], [`Segment`], and
//! [`WhisperResult`].
//!
//! Mirrors `stable_whisper.result` (the upstream `result.py`) closely enough
//! that the B (regroup), C (suppress-silence), and D (output) stages can build
//! on it. The pieces ported here are the *data* shape and the pure accessors:
//!
//! * **Fields** — every constructor field stable-ts keeps on each struct.
//! * **Derived accessors** — a [`Segment`]'s `start`/`end`/`text`/`tokens`
//!   come from its words when it has any, else from its stored defaults; a
//!   [`WhisperResult`]'s `text` is the concatenation of its segments' text.
//! * **Locking flags** — per-word `left_locked`/`right_locked` plus the
//!   `lock_*`/`unlock_*` helpers regroup relies on.
//! * **3-decimal timestamp rounding** — stable-ts rounds every timestamp to 3
//!   decimals with Python's round-half-to-even ([`round_timestamp`]); the
//!   serde `to_dict` representation reproduces that so the JSON matches the
//!   Python golden.
//!
//! ## Serde shape (`to_dict` parity)
//!
//! [`WhisperResult::to_dict`] reproduces `WhisperResult.to_dict()`:
//! `{text, segments, language, ori_dict, regroup_history, nonspeech_sections}`,
//! where each segment is `{start, end, text, seek, tokens, temperature,
//! avg_logprob, compression_ratio, no_speech_prob, words?}` and each word is
//! `{word, start, end, probability, tokens}`. Parsing a captured `to_dict()`
//! JSON and re-emitting it reproduces the same JSON value (see the
//! `parity::model_roundtrip` falsifier).
//!
//! `ori_dict` and `seek` are kept as raw [`serde_json::Value`] so the exact
//! original numeric form (e.g. an integer `seek`, or the untouched original
//! result dict) round-trips without being coerced to `f64`.

use serde::Deserialize;
use serde_json::{Map, Value};

/// Round a timestamp the way stable-ts's `_round_timestamp` does.
///
/// Upstream is `round(ts, 3)` guarded by `if not ts`, so a `0.0` (or any
/// falsy value) is returned untouched. Python's `round` is round-half-to-even
/// ("banker's rounding"); we reproduce that rather than the usual
/// round-half-away-from-zero so re-rounding already-rounded goldens is a no-op
/// and so split/merge arithmetic agrees with Python downstream.
#[must_use]
pub fn round_timestamp(ts: f64) -> f64 {
    if ts == 0.0 || !ts.is_finite() {
        return ts;
    }
    round_half_even(ts, 3)
}

/// Round `value` to `ndigits` decimal places, half-to-even, matching Python's
/// built-in `round` for finite floats.
fn round_half_even(value: f64, ndigits: i32) -> f64 {
    let factor = 10f64.powi(ndigits);
    let scaled = value * factor;
    let rounded = scaled.round_ties_even();
    rounded / factor
}

/// A single word with its timing, mirroring `stable_whisper.result.WordTiming`.
///
/// `start`/`end` are stored already rounded (see [`round_timestamp`]); use the
/// constructors / setters rather than touching the fields to keep that
/// invariant.
#[derive(Debug, Clone, PartialEq)]
pub struct WordTiming {
    /// The word text, including any leading space stable-ts keeps.
    pub word: String,
    start: f64,
    end: f64,
    /// Model probability for the word, if known.
    pub probability: Option<f64>,
    /// Subword token ids, if carried through; `None` is distinct from `[]`.
    pub tokens: Option<Vec<i64>>,
    /// Regroup lock: this word may not merge with the previous one.
    pub left_locked: bool,
    /// Regroup lock: this word may not merge with the next one.
    pub right_locked: bool,
    /// Word id within its segment, assigned by `reassign_ids`.
    pub id: Option<i64>,
}

impl WordTiming {
    /// Build a word, rounding `start`/`end` like stable-ts does on construction.
    #[must_use]
    pub fn new(word: impl Into<String>, start: f64, end: f64) -> Self {
        WordTiming {
            word: word.into(),
            start: round_timestamp(start),
            end: round_timestamp(end),
            probability: None,
            tokens: None,
            left_locked: false,
            right_locked: false,
            id: None,
        }
    }

    /// Rounded start timestamp.
    #[must_use]
    pub fn start(&self) -> f64 {
        self.start
    }

    /// Rounded end timestamp.
    #[must_use]
    pub fn end(&self) -> f64 {
        self.end
    }

    /// Set the start timestamp, rounding it (matches the Python `start` setter).
    pub fn set_start(&mut self, val: f64) {
        self.start = round_timestamp(val);
    }

    /// Set the end timestamp, rounding it (matches the Python `end` setter).
    pub fn set_end(&mut self, val: f64) {
        self.end = round_timestamp(val);
    }

    /// Rounded duration, mirroring the Python `duration` property.
    #[must_use]
    pub fn duration(&self) -> f64 {
        round_timestamp(self.end - self.start)
    }

    /// `lock_left` — forbid merging with the previous word.
    pub fn lock_left(&mut self) {
        self.left_locked = true;
    }

    /// `lock_right` — forbid merging with the next word.
    pub fn lock_right(&mut self) {
        self.right_locked = true;
    }

    /// `lock_both`.
    pub fn lock_both(&mut self) {
        self.lock_left();
        self.lock_right();
    }

    /// `unlock_both`.
    pub fn unlock_both(&mut self) {
        self.left_locked = false;
        self.right_locked = false;
    }

    /// Serialize like `WordTiming.to_dict()`:
    /// `{word, start, end, probability, tokens}`.
    #[must_use]
    pub fn to_dict(&self) -> Value {
        let mut map = Map::new();
        map.insert("word".into(), Value::String(self.word.clone()));
        map.insert("start".into(), number(self.start));
        map.insert("end".into(), number(self.end));
        map.insert("probability".into(), opt_number(self.probability));
        map.insert("tokens".into(), opt_int_array(self.tokens.as_deref()));
        Value::Object(map)
    }
}

/// Raw word as it appears inside a captured `to_dict()` segment.
#[derive(Debug, Deserialize)]
struct RawWord {
    word: String,
    start: f64,
    end: f64,
    #[serde(default)]
    probability: Option<f64>,
    #[serde(default)]
    tokens: Option<Vec<i64>>,
    #[serde(default)]
    left_locked: bool,
    #[serde(default)]
    right_locked: bool,
    #[serde(default)]
    id: Option<i64>,
}

impl From<RawWord> for WordTiming {
    fn from(r: RawWord) -> Self {
        WordTiming {
            word: r.word,
            start: round_timestamp(r.start),
            end: round_timestamp(r.end),
            probability: r.probability,
            tokens: r.tokens,
            left_locked: r.left_locked,
            right_locked: r.right_locked,
            id: r.id,
        }
    }
}

/// A transcription segment, mirroring `stable_whisper.result.Segment`.
///
/// When `words` is `Some` and non-empty, `start`/`end`/`text`/`tokens` are
/// derived from the words; otherwise the `default_*` fields supply them. This
/// matches the Python properties exactly, including that an empty `words`
/// vector falls back to the defaults (`has_words` is false for `[]`).
#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    default_start: f64,
    default_end: f64,
    default_text: String,
    default_tokens: Vec<i64>,
    /// Original `seek` value, kept verbatim so an integer seek round-trips.
    pub seek: Option<Value>,
    /// Sampling temperature reported for the segment.
    pub temperature: Option<f64>,
    /// Average log-probability over the segment.
    pub avg_logprob: Option<f64>,
    /// Compression ratio of the segment text.
    pub compression_ratio: Option<f64>,
    /// Probability the segment is non-speech.
    pub no_speech_prob: Option<f64>,
    /// Word timings, or `None` when the segment carries no word-level data.
    pub words: Option<Vec<WordTiming>>,
    /// Segment id, assigned by `reassign_ids`.
    pub id: Option<i64>,
}

impl Segment {
    /// `has_words`: true only for a non-empty word list (`bool(self.words)`).
    #[must_use]
    pub fn has_words(&self) -> bool {
        self.words.as_ref().is_some_and(|w| !w.is_empty())
    }

    /// `ori_has_words`: the segment originally carried a word list, even if
    /// it is now empty (`self.words is not None`).
    #[must_use]
    pub fn ori_has_words(&self) -> bool {
        self.words.is_some()
    }

    /// Derived `start`: first word's start, else the default.
    #[must_use]
    pub fn start(&self) -> f64 {
        match self.words.as_ref() {
            Some(w) if !w.is_empty() => w[0].start(),
            _ => self.default_start,
        }
    }

    /// Derived `end`: last word's end, else the default.
    #[must_use]
    pub fn end(&self) -> f64 {
        match self.words.as_ref() {
            Some(w) if !w.is_empty() => w[w.len() - 1].end(),
            _ => self.default_end,
        }
    }

    /// Derived `text`: concatenation of the words' text, else the default.
    #[must_use]
    pub fn text(&self) -> String {
        match self.words.as_ref() {
            Some(w) if !w.is_empty() => w.iter().map(|x| x.word.as_str()).collect(),
            _ => self.default_text.clone(),
        }
    }

    /// Derived `tokens`: concatenation of the words' tokens when the first word
    /// carries tokens, else the default token list. Mirrors the Python guard
    /// `if self.has_words and self.words[0].tokens`.
    #[must_use]
    pub fn tokens(&self) -> Vec<i64> {
        if let Some(w) = self.words.as_ref() {
            if let Some(first) = w.first() {
                if first.tokens.as_ref().is_some_and(|t| !t.is_empty()) {
                    return w
                        .iter()
                        .filter_map(|x| x.tokens.as_ref())
                        .flatten()
                        .copied()
                        .collect();
                }
            }
        }
        self.default_tokens.clone()
    }

    /// `left_locked`: first word's flag, false when there are no words.
    #[must_use]
    pub fn left_locked(&self) -> bool {
        match self.words.as_ref() {
            Some(w) if !w.is_empty() => w[0].left_locked,
            _ => false,
        }
    }

    /// `right_locked`: last word's flag, false when there are no words.
    #[must_use]
    pub fn right_locked(&self) -> bool {
        match self.words.as_ref() {
            Some(w) if !w.is_empty() => w[w.len() - 1].right_locked,
            _ => false,
        }
    }

    /// `lock_left`: lock the first word's left edge.
    pub fn lock_left(&mut self) {
        if let Some(w) = self.words.as_mut() {
            if let Some(first) = w.first_mut() {
                first.lock_left();
            }
        }
    }

    /// `lock_right`: lock the last word's right edge.
    pub fn lock_right(&mut self) {
        if let Some(w) = self.words.as_mut() {
            if let Some(last) = w.last_mut() {
                last.lock_right();
            }
        }
    }

    /// `lock_both`.
    pub fn lock_both(&mut self) {
        self.lock_left();
        self.lock_right();
    }

    /// `unlock_all_words`: clear both locks on every word.
    pub fn unlock_all_words(&mut self) {
        if let Some(w) = self.words.as_mut() {
            for word in w.iter_mut() {
                word.unlock_both();
            }
        }
    }

    /// Serialize like `Segment.to_dict()`. Emits `words` as a (possibly empty)
    /// array when the segment originally had words, and omits the key entirely
    /// otherwise.
    #[must_use]
    pub fn to_dict(&self) -> Value {
        let mut map = Map::new();
        map.insert("start".into(), number(self.start()));
        map.insert("end".into(), number(self.end()));
        map.insert("text".into(), Value::String(self.text()));
        map.insert("seek".into(), self.seek.clone().unwrap_or(Value::Null));
        map.insert("tokens".into(), int_array(&self.tokens()));
        map.insert("temperature".into(), opt_number(self.temperature));
        map.insert("avg_logprob".into(), opt_number(self.avg_logprob));
        map.insert("compression_ratio".into(), opt_number(self.compression_ratio));
        map.insert("no_speech_prob".into(), opt_number(self.no_speech_prob));
        if self.has_words() {
            let words = self.words.as_ref().expect("has_words implies Some");
            map.insert(
                "words".into(),
                Value::Array(words.iter().map(WordTiming::to_dict).collect()),
            );
        } else if self.ori_has_words() {
            map.insert("words".into(), Value::Array(Vec::new()));
        }
        Value::Object(map)
    }
}

/// Raw segment as it appears inside a captured `to_dict()` / input dict.
#[derive(Debug, Deserialize)]
struct RawSegment {
    #[serde(default)]
    start: Option<f64>,
    #[serde(default)]
    end: Option<f64>,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    seek: Option<Value>,
    #[serde(default)]
    tokens: Option<Vec<i64>>,
    #[serde(default)]
    temperature: Option<f64>,
    #[serde(default)]
    avg_logprob: Option<f64>,
    #[serde(default)]
    compression_ratio: Option<f64>,
    #[serde(default)]
    no_speech_prob: Option<f64>,
    #[serde(default)]
    words: Option<Vec<RawWord>>,
    #[serde(default)]
    id: Option<i64>,
}

impl From<RawSegment> for Segment {
    fn from(r: RawSegment) -> Self {
        // Python: `self.round(start) if start else 0.0` — a falsy (0.0/None)
        // start collapses to 0.0, otherwise it is rounded.
        let default_start = r.start.filter(|&s| s != 0.0).map_or(0.0, round_timestamp);
        let default_end = r.end.filter(|&e| e != 0.0).map_or(0.0, round_timestamp);
        Segment {
            default_start,
            default_end,
            default_text: r.text.unwrap_or_default(),
            default_tokens: r.tokens.unwrap_or_default(),
            seek: r.seek,
            temperature: r.temperature,
            avg_logprob: r.avg_logprob,
            compression_ratio: r.compression_ratio,
            no_speech_prob: r.no_speech_prob,
            words: r.words.map(|ws| ws.into_iter().map(WordTiming::from).collect()),
            id: r.id,
        }
    }
}

/// The full transcription result, mirroring
/// `stable_whisper.result.WhisperResult`.
#[derive(Debug, Clone, PartialEq)]
pub struct WhisperResult {
    /// Parsed segments.
    pub segments: Vec<Segment>,
    /// Detected language, taken from `ori_dict`.
    pub language: Option<String>,
    /// The original result dict, kept verbatim for `to_dict(keep_orig=True)`.
    pub ori_dict: Value,
    /// Regroup operation history string.
    pub regroup_history: String,
    /// Non-speech sections, kept verbatim.
    pub nonspeech_sections: Value,
}

impl WhisperResult {
    /// Parse a captured `to_dict()` JSON value into a [`WhisperResult`].
    ///
    /// Mirrors `WhisperResult.__init__` for the dict form: `ori_dict` is the
    /// nested `ori_dict` if present else the whole input; `language` comes from
    /// `ori_dict`; segments come from the top-level `segments` else
    /// `ori_dict.segments`.
    #[must_use]
    pub fn from_value(input: &Value) -> Self {
        let obj = input.as_object().cloned().unwrap_or_default();

        let ori_dict = match obj.get("ori_dict") {
            Some(Value::Null) | None => input.clone(),
            Some(v) => v.clone(),
        };
        let language = ori_dict
            .get("language")
            .and_then(Value::as_str)
            .map(str::to_owned);
        let regroup_history = obj
            .get("regroup_history")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned();
        let nonspeech_sections = obj
            .get("nonspeech_sections")
            .cloned()
            .unwrap_or_else(|| Value::Array(Vec::new()));

        let raw_segments = match obj.get("segments") {
            Some(v @ Value::Array(_)) => v.clone(),
            _ => ori_dict
                .get("segments")
                .cloned()
                .unwrap_or_else(|| Value::Array(Vec::new())),
        };
        let segments: Vec<Segment> = serde_json::from_value::<Vec<RawSegment>>(raw_segments)
            .expect("segments array of objects")
            .into_iter()
            .map(Segment::from)
            .collect();

        WhisperResult { segments, language, ori_dict, regroup_history, nonspeech_sections }
    }

    /// Derived `text`: concatenation of every segment's text.
    #[must_use]
    pub fn text(&self) -> String {
        self.segments.iter().map(Segment::text).collect()
    }

    /// Serialize like `WhisperResult.to_dict(keep_orig=True)`.
    #[must_use]
    pub fn to_dict(&self) -> Value {
        let mut map = Map::new();
        map.insert("text".into(), Value::String(self.text()));
        map.insert(
            "segments".into(),
            Value::Array(self.segments.iter().map(Segment::to_dict).collect()),
        );
        map.insert(
            "language".into(),
            self.language
                .as_ref()
                .map_or(Value::Null, |s| Value::String(s.clone())),
        );
        map.insert("ori_dict".into(), self.ori_dict.clone());
        map.insert("regroup_history".into(), Value::String(self.regroup_history.clone()));
        map.insert("nonspeech_sections".into(), self.nonspeech_sections.clone());
        Value::Object(map)
    }
}

/// Wrap a finite `f64` as a JSON number, matching how Python's `json` emits a
/// float (always a JSON number, never null for `NaN`/`inf` here since timings
/// are finite). Falls back to `Null` for non-finite values.
fn number(v: f64) -> Value {
    serde_json::Number::from_f64(v).map_or(Value::Null, Value::Number)
}

fn opt_number(v: Option<f64>) -> Value {
    v.map_or(Value::Null, number)
}

fn int_array(tokens: &[i64]) -> Value {
    Value::Array(tokens.iter().map(|&t| Value::Number(t.into())).collect())
}

fn opt_int_array(tokens: Option<&[i64]>) -> Value {
    tokens.map_or(Value::Null, int_array)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_timestamp_matches_python_banker_rounding() {
        assert_eq!(round_timestamp(0.0), 0.0);
        assert_eq!(round_timestamp(5.9), 5.9);
        assert_eq!(round_timestamp(0.14), 0.14);
        // Round-half-to-even: 0.0005 -> 0.0 (Python `round(0.0005, 3)` == 0.0).
        assert_eq!(round_timestamp(0.0015), 0.002);
        // Re-rounding an already 3-decimal value is a no-op.
        assert_eq!(round_timestamp(0.123), 0.123);
    }

    #[test]
    fn segment_derives_from_words() {
        let mut w0 = WordTiming::new(" Hello", 0.0, 0.5);
        w0.tokens = Some(vec![1, 2]);
        let w1 = WordTiming::new(" world", 0.5, 1.0);
        let seg = Segment {
            default_start: 9.0,
            default_end: 9.0,
            default_text: "ignored".into(),
            default_tokens: vec![99],
            seek: Some(Value::from(0)),
            temperature: Some(0.0),
            avg_logprob: None,
            compression_ratio: None,
            no_speech_prob: None,
            words: Some(vec![w0, w1]),
            id: None,
        };
        assert!(seg.has_words());
        assert_eq!(seg.start(), 0.0);
        assert_eq!(seg.end(), 1.0);
        assert_eq!(seg.text(), " Hello world");
        // First word has tokens, so tokens flatten across words.
        assert_eq!(seg.tokens(), vec![1, 2]);
    }

    #[test]
    fn empty_words_falls_back_to_defaults_but_emits_words() {
        let seg = Segment {
            default_start: 1.0,
            default_end: 2.0,
            default_text: "default".into(),
            default_tokens: vec![7],
            seek: None,
            temperature: None,
            avg_logprob: None,
            compression_ratio: None,
            no_speech_prob: None,
            words: Some(Vec::new()),
            id: None,
        };
        assert!(!seg.has_words());
        assert!(seg.ori_has_words());
        assert_eq!(seg.text(), "default");
        assert_eq!(seg.start(), 1.0);
        let dict = seg.to_dict();
        assert_eq!(dict["words"], Value::Array(Vec::new()));
        assert_eq!(dict["tokens"], int_array(&[7]));
    }
}
