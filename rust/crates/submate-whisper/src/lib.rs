//! whisper-rs transcription pipeline (ports submate/whisper.py).
//!
//! Wraps native whisper.cpp inference (via `whisper-rs`) and assembles a
//! stable-ts-shaped [`WhisperResult`] carrying per-word timestamps, which the
//! stable-ts slice (regroup / suppress_silence / output) consumes.
//!
//! Real model execution is gated behind the `model` cargo feature. That keeps
//! whisper.cpp (which needs `LIBCLANG_PATH`/`cmake` from the devshell) out of
//! the default build, so CI without a model still compiles the crate and runs
//! the non-model tests.

use serde::{Deserialize, Serialize};

/// A single recognized word and the time span it occupies, in seconds.
///
/// Mirrors the per-word entries stable-whisper attaches to each segment
/// (`WordTiming`): `word`, `start`, `end`, plus the model's average token
/// probability for the word.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WhisperWord {
    /// Word text, including any leading space the tokenizer emitted.
    pub word: String,
    /// Word start time, in seconds from the clip origin.
    pub start: f64,
    /// Word end time, in seconds from the clip origin.
    pub end: f64,
    /// Mean token probability across the word's tokens, in `0.0..=1.0`.
    pub probability: f64,
}

/// One transcription segment: a contiguous run of words with its own span.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WhisperSegment {
    /// Segment text (concatenation of its words).
    pub text: String,
    /// Segment start time, in seconds.
    pub start: f64,
    /// Segment end time, in seconds.
    pub end: f64,
    /// Per-word timings within the segment.
    pub words: Vec<WhisperWord>,
}

/// The full transcription result, shaped like stable-whisper's `WhisperResult`.
///
/// `language` is the detected (or forced) language code, `text` is the joined
/// segment text, and `segments` carries the per-word timings downstream slices
/// rely on.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WhisperResult {
    /// Detected or forced language code (e.g. `"en"`).
    pub language: String,
    /// Joined transcript text across all segments.
    pub text: String,
    /// Ordered transcription segments.
    pub segments: Vec<WhisperSegment>,
}

impl WhisperResult {
    /// Total number of words across all segments.
    pub fn word_count(&self) -> usize {
        self.segments.iter().map(|s| s.words.len()).sum()
    }
}

/// What to do with the audio: transcribe in-language, or translate to English.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Task {
    /// Transcribe in the spoken language.
    #[default]
    Transcribe,
    /// Translate the speech to English.
    Translate,
}

/// Options controlling a single transcription run.
#[derive(Debug, Clone, Default)]
pub struct TranscribeOptions {
    /// Forced language code, or `None` to auto-detect.
    pub language: Option<String>,
    /// Transcribe or translate.
    pub task: Task,
}

/// Errors raised while loading a model or running inference.
#[derive(Debug, thiserror::Error)]
pub enum WhisperError {
    /// The model file path does not point at a readable file.
    #[error("model not found: {0}")]
    ModelNotFound(String),
    /// Loading the model into a whisper context failed.
    #[error("failed to load model: {0}")]
    Load(String),
    /// Running inference failed.
    #[error("transcription failed: {0}")]
    Inference(String),
    /// The blocking inference task panicked or was cancelled.
    #[error("transcription task did not complete: {0}")]
    Join(String),
}

/// PCM sample rate expected by whisper.cpp: 16 kHz, mono, f32 in `-1.0..=1.0`.
pub const SAMPLE_RATE: u32 = 16_000;

/// A whisper.cpp token's raw bytes + timing — the input to word grouping.
///
/// `t0`/`t1` are whisper.cpp centiseconds. `bytes` are the token's raw UTF-8
/// bytes, which for byte-fallback tokens (routine for CJK) are only a *fragment*
/// of a multibyte character; grouping concatenates them so the character
/// reassembles. (Gated on `model`-or-`test` so it isn't dead code in the
/// default, model-less build.)
#[cfg(any(feature = "model", test))]
struct RawToken {
    bytes: Vec<u8>,
    t0: i64,
    t1: i64,
    prob: f64,
}

/// Whether `c` belongs to a CJK script (Han, kana, Hangul, CJK
/// punctuation/fullwidth). These scripts have no inter-word spaces, so each such
/// character is treated as its own timed "word" — otherwise a whole Japanese
/// segment collapses into a single word that can never be split.
#[cfg(any(feature = "model", test))]
fn is_cjk(c: char) -> bool {
    matches!(c as u32,
        0x3000..=0x303F   // CJK symbols & punctuation (。、「」 …)
        | 0x3040..=0x30FF // Hiragana + Katakana
        | 0x3400..=0x4DBF // CJK Unified Ext A
        | 0x4E00..=0x9FFF // CJK Unified Ideographs
        | 0xF900..=0xFAFF // CJK Compatibility Ideographs
        | 0xFF00..=0xFFEF // Halfwidth/Fullwidth forms
        | 0xAC00..=0xD7AF // Hangul syllables
    )
}

/// A new word starts at `cur` when it is a leading space (whisper's `" word"`
/// tokenization), a CJK character, or the first character after a CJK one.
#[cfg(any(feature = "model", test))]
fn starts_new_word(cur: char, prev: char) -> bool {
    cur.is_whitespace() || is_cjk(cur) || is_cjk(prev)
}

/// Aggregate timing/probability over the tokens whose byte span overlaps a
/// word's byte range `[byte_start, byte_end)`. Returns `(start_s, end_s, prob)`.
#[cfg(any(feature = "model", test))]
fn word_timing(
    spans: &[(usize, usize, i64, i64, f64)],
    byte_start: usize,
    byte_end: usize,
) -> (f64, f64, f64) {
    let (mut t0, mut t1): (Option<i64>, Option<i64>) = (None, None);
    let (mut prob_sum, mut prob_count) = (0.0_f64, 0_u32);
    for &(ts, te, tok_t0, tok_t1, prob) in spans {
        if ts < byte_end && te > byte_start {
            t0 = Some(t0.map_or(tok_t0, |x| x.min(tok_t0)));
            t1 = Some(t1.map_or(tok_t1, |x| x.max(tok_t1)));
            prob_sum += prob;
            prob_count += 1;
        }
    }
    let to_s = |cs: i64| cs as f64 / 100.0;
    let prob = if prob_count > 0 {
        prob_sum / f64::from(prob_count)
    } else {
        0.0
    };
    (to_s(t0.unwrap_or(0)), to_s(t1.unwrap_or(0)), prob)
}

/// Group whisper.cpp tokens into words with per-word timing.
///
/// whisper.cpp emits byte-fallback tokens that split one multibyte UTF-8
/// character (e.g. a kanji) across several tokens, so tokens cannot be decoded
/// individually. This concatenates every token's bytes, decodes the whole
/// segment, then splits it into words — at each leading space for
/// space-delimited scripts, and per character for CJK — and aggregates each
/// word's timing from the tokens its bytes cover.
#[cfg(any(feature = "model", test))]
fn group_tokens_into_words(tokens: &[RawToken]) -> Vec<WhisperWord> {
    let mut buf: Vec<u8> = Vec::new();
    let mut spans: Vec<(usize, usize, i64, i64, f64)> = Vec::with_capacity(tokens.len());
    for t in tokens {
        let start = buf.len();
        buf.extend_from_slice(&t.bytes);
        spans.push((start, buf.len(), t.t0, t.t1, t.prob));
    }

    // Byte-fallback fragments reassemble into valid UTF-8 here; a genuinely
    // malformed run degrades to U+FFFD instead of losing every word.
    let text = String::from_utf8_lossy(&buf).into_owned();
    let chars: Vec<(usize, char)> = text.char_indices().collect();

    let mut words: Vec<WhisperWord> = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let word_start = chars[i].0;
        let mut j = i + 1;
        while j < chars.len() && !starts_new_word(chars[j].1, chars[j - 1].1) {
            j += 1;
        }
        let word_end = chars.get(j).map_or(buf.len(), |&(b, _)| b);
        let (start, end, probability) = word_timing(&spans, word_start, word_end);
        words.push(WhisperWord {
            word: text[word_start..word_end].to_string(),
            start,
            end,
            probability,
        });
        i = j;
    }
    words
}

#[cfg(test)]
mod word_grouping_tests {
    use super::{group_tokens_into_words, RawToken};

    fn tok(bytes: &[u8], t0: i64, t1: i64) -> RawToken {
        RawToken { bytes: bytes.to_vec(), t0, t1, prob: 1.0 }
    }
    fn texts(words: &[super::WhisperWord]) -> Vec<&str> {
        words.iter().map(|w| w.word.as_str()).collect()
    }

    #[test]
    fn latin_splits_on_leading_space() {
        let words = group_tokens_into_words(&[tok(b" hello", 0, 50), tok(b" world", 50, 100)]);
        assert_eq!(texts(&words), vec![" hello", " world"]);
        assert_eq!(words[0].start, 0.0);
        assert_eq!(words[1].end, 1.0);
    }

    #[test]
    fn cjk_char_split_across_byte_fallback_tokens_reassembles() {
        // 水 = E6 B0 B4, を = E3 82 92, each emitted as three byte-fallback tokens.
        let words = group_tokens_into_words(&[
            tok(&[0xE6], 0, 10),
            tok(&[0xB0], 10, 20),
            tok(&[0xB4], 20, 30),
            tok(&[0xE3], 30, 40),
            tok(&[0x82], 40, 50),
            tok(&[0x92], 50, 60),
        ]);
        assert_eq!(texts(&words), vec!["水", "を"]);
        assert_eq!((words[0].start, words[0].end), (0.0, 0.3));
        assert_eq!((words[1].start, words[1].end), (0.3, 0.6));
    }

    #[test]
    fn whole_cjk_tokens_become_per_char_words() {
        let words =
            group_tokens_into_words(&[tok("水".as_bytes(), 0, 30), tok("を".as_bytes(), 30, 60)]);
        assert_eq!(texts(&words), vec!["水", "を"]);
    }
}

#[cfg(feature = "model")]
mod inference {
    use super::*;
    use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

    use std::collections::HashMap;
    use std::sync::{Arc, Mutex, OnceLock};

    /// whisper.cpp reports token/segment times in centiseconds (1/100 s).
    fn centiseconds_to_seconds(cs: i64) -> f64 {
        cs as f64 / 100.0
    }

    /// Process-wide cache of loaded whisper models, keyed by model file path.
    ///
    /// `WhisperContext::new_with_params` parses and loads the entire GGML model
    /// (hundreds of MB) — doing it per job dominates short-clip latency and is
    /// pure waste when a node drains many jobs against the same model. The
    /// context is `Send + Sync` and `create_state` is cheap and per-call, so we
    /// load each model once and share an `Arc` across all jobs.
    fn context_cache() -> &'static Mutex<HashMap<String, Arc<WhisperContext>>> {
        static CACHE: OnceLock<Mutex<HashMap<String, Arc<WhisperContext>>>> = OnceLock::new();
        CACHE.get_or_init(|| Mutex::new(HashMap::new()))
    }

    /// Return the cached context for `model_path`, loading and caching it on
    /// first use. The load holds the cache lock, so a cold-start race serializes
    /// on the first load and the loser reuses the freshly cached context — both
    /// correct and a one-time cost.
    fn load_context(model_path: &str) -> Result<Arc<WhisperContext>, WhisperError> {
        let mut cache = context_cache()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(ctx) = cache.get(model_path) {
            return Ok(Arc::clone(ctx));
        }
        tracing::debug!(model = model_path, "loading whisper model (cache miss)");
        let ctx = Arc::new(
            WhisperContext::new_with_params(model_path, WhisperContextParameters::default())
                .map_err(|e| WhisperError::Load(e.to_string()))?,
        );
        cache.insert(model_path.to_string(), Arc::clone(&ctx));
        Ok(ctx)
    }

    /// Optional whisper.cpp thread-count override from `SUBMATE_WHISPER_THREADS`.
    ///
    /// Returns `None` (leave whisper.cpp's own default of `min(4, n_cpu)`) unless
    /// the env var is set. Measured on a 20-thread box with the `base` model,
    /// raising the thread count above the default *regresses* (4→27s, 8→37s,
    /// 20→113s): inference is memory-bandwidth-bound, so oversubscription
    /// thrashes. The optimum is model- and host-dependent (a large model on many
    /// physical cores may benefit), so we expose it as a knob instead of forcing
    /// a value that helps in theory but hurts in practice.
    fn whisper_threads() -> Option<std::os::raw::c_int> {
        std::env::var("SUBMATE_WHISPER_THREADS")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .map(clamp_threads)
    }

    /// Clamp a core count into a valid `set_n_threads` argument (`>= 1`, fits
    /// `c_int`). Split out so the bounds logic is testable without depending on
    /// the host's core count.
    fn clamp_threads(cores: usize) -> std::os::raw::c_int {
        cores.clamp(1, std::os::raw::c_int::MAX as usize) as std::os::raw::c_int
    }

    /// Load a whisper model and transcribe a mono 16 kHz f32 PCM clip.
    ///
    /// The heavy whisper.cpp work runs on a blocking thread via
    /// [`tokio::task::spawn_blocking`], so this is safe to call from an async
    /// context without stalling the runtime.
    pub async fn transcribe_pcm(
        model_path: impl Into<String>,
        pcm: Vec<f32>,
        options: TranscribeOptions,
    ) -> Result<WhisperResult, WhisperError> {
        let model_path = model_path.into();
        if !std::path::Path::new(&model_path).is_file() {
            return Err(WhisperError::ModelNotFound(model_path));
        }

        tokio::task::spawn_blocking(move || transcribe_blocking(&model_path, &pcm, &options))
            .await
            .map_err(|e| WhisperError::Join(e.to_string()))?
    }

    fn transcribe_blocking(
        model_path: &str,
        pcm: &[f32],
        options: &TranscribeOptions,
    ) -> Result<WhisperResult, WhisperError> {
        let ctx = load_context(model_path)?;

        let mut state = ctx
            .create_state()
            .map_err(|e| WhisperError::Load(e.to_string()))?;

        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        // Only override whisper.cpp's default thread count when explicitly asked
        // (SUBMATE_WHISPER_THREADS) — forcing more threads regresses small models.
        if let Some(threads) = whisper_threads() {
            params.set_n_threads(threads);
        }
        // Word-level timestamps: ask whisper.cpp to emit per-token times so we
        // can fold tokens into words below.
        params.set_token_timestamps(true);
        params.set_translate(matches!(options.task, Task::Translate));
        if let Some(lang) = options.language.as_deref() {
            params.set_language(Some(lang));
        }
        // Quiet: whisper.cpp's stdout callbacks have no place in a library.
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);

        state
            .full(params, pcm)
            .map_err(|e| WhisperError::Inference(e.to_string()))?;

        let n_segments = state.full_n_segments();

        let mut segments = Vec::with_capacity(n_segments.max(0) as usize);
        let mut full_text = String::new();

        for seg in 0..n_segments {
            let Some(segment) = state.get_segment(seg) else {
                continue;
            };
            // `to_str_lossy`: the full segment's bytes reassemble into valid
            // UTF-8 (whisper.cpp's byte-fallback tokens combine at the segment
            // level), but the lossy reader degrades a rare malformed segment to
            // `U+FFFD` instead of failing the whole transcription.
            let seg_text = segment
                .to_str_lossy()
                .map_err(|e| WhisperError::Inference(e.to_string()))?
                .into_owned();
            let seg_t0 = segment.start_timestamp();
            let seg_t1 = segment.end_timestamp();

            full_text.push_str(&seg_text);

            let words = collect_words(&segment)?;
            segments.push(WhisperSegment {
                text: seg_text,
                start: centiseconds_to_seconds(seg_t0),
                end: centiseconds_to_seconds(seg_t1),
                words,
            });
        }

        Ok(WhisperResult {
            language: detect_language(&state),
            text: full_text.trim().to_string(),
            segments,
        })
    }

    /// Fold a segment's tokens into words.
    ///
    /// Collects each token's raw bytes + timing (skipping special tokens) and
    /// delegates to [`group_tokens_into_words`], which reassembles byte-fallback
    /// fragments into characters and splits into words — per leading space for
    /// space-delimited scripts, per character for CJK. Using the raw bytes (not
    /// the per-token text API, which errors on a multibyte fragment) is what
    /// gives CJK real word-level timing instead of collapsing to segment level.
    fn collect_words(
        segment: &whisper_rs::WhisperSegment<'_>,
    ) -> Result<Vec<WhisperWord>, WhisperError> {
        let n_tokens = segment.n_tokens();
        let mut raw: Vec<RawToken> = Vec::with_capacity(n_tokens.max(0) as usize);

        for tok in 0..n_tokens {
            let Some(token) = segment.get_token(tok) else {
                continue;
            };
            // Skip special tokens (`[_BEG_]`, `[_EOT_]`, …) by their decoded
            // form. A byte-fallback fragment decodes lossily to `U+FFFD`, which
            // never matches the `[_…]` shape, so real text tokens are kept.
            let lossy = token
                .to_str_lossy()
                .map_err(|e| WhisperError::Inference(e.to_string()))?;
            if lossy.starts_with("[_") && lossy.ends_with(']') {
                continue;
            }
            let bytes = token
                .to_bytes()
                .map_err(|e| WhisperError::Inference(e.to_string()))?
                .to_vec();
            let data = token.token_data();
            raw.push(RawToken {
                bytes,
                t0: data.t0,
                t1: data.t1,
                prob: f64::from(data.p),
            });
        }

        Ok(group_tokens_into_words(&raw))
    }

    fn detect_language(state: &whisper_rs::WhisperState) -> String {
        whisper_rs::get_lang_str(state.full_lang_id_from_state())
            .unwrap_or("en")
            .to_string()
    }

    #[cfg(test)]
    mod tests {
        use super::clamp_threads;

        #[test]
        fn clamp_threads_stays_positive_and_in_range() {
            assert_eq!(clamp_threads(0), 1, "0 cores must clamp up to 1");
            assert_eq!(clamp_threads(1), 1);
            assert_eq!(clamp_threads(8), 8);
            assert_eq!(clamp_threads(64), 64);
            // Absurd counts saturate at c_int::MAX rather than wrapping negative.
            assert_eq!(clamp_threads(usize::MAX), std::os::raw::c_int::MAX);
        }
    }
}

#[cfg(feature = "model")]
pub use inference::transcribe_pcm;

/// The submate config regroup string this pipeline drives by default.
///
/// Mirrors `StableTsSettings.custom_regroup`'s default
/// (`config.py`): a `clamp_max` followed by two `split_by_length` passes. The
/// [`transcribe`] / [`assemble_result`] entry points take it as a parameter so
/// callers can pass their resolved config value (or `""`/`None` to skip
/// regrouping), but this constant documents the shipped default.
pub const DEFAULT_REGROUP: &str = "cm_sl=84_sl=42++++++1";

/// `min_word_dur` the suppress-silence stage uses, matching
/// `StableTsSettings.min_word_duration`'s default and
/// `transcribe_stable`'s `min_word_dur=0.1`.
pub const DEFAULT_MIN_WORD_DUR: f64 = 0.1;

/// `nonspeech_error` `transcribe_stable` passes to `suppress_silence`.
pub const DEFAULT_NONSPEECH_ERROR: f64 = 0.1;

/// Errors raised while assembling the post-inference pipeline (regroup →
/// suppress → output).
#[derive(Debug, thiserror::Error)]
pub enum PipelineError {
    /// The regroup string named a method the parser does not know.
    #[error("unknown regroup method: {0}")]
    UnknownRegroupMethod(String),
    /// A parsed regroup op is not yet runnable in the ported regroup engine.
    #[error("unsupported regroup method: {0}")]
    UnsupportedRegroupMethod(String),
}

/// The finished transcription, shaped like the Python `TranscriptionResult`
/// the CLI and queue consume: `.text`, `.language`, `.segments`, and
/// `.to_srt_vtt()`.
///
/// Built by [`assemble_result`] from a raw [`WhisperResult`] (whisper.cpp
/// inference output) by running the same post-decode stages
/// `WhisperModelWrapper.transcribe` runs: regroup, then suppress-silence, then
/// SRT/VTT rendering. The stages live in the ported sibling crates
/// (`stable_ts`); this crate only wires them.
#[derive(Debug, Clone)]
pub struct Transcription {
    /// Detected (or forced) language code, e.g. `"english"`/`"en"`.
    pub language: Option<String>,
    /// The post-stage [`stable_ts::WhisperResult`], carrying the regrouped /
    /// suppressed segments and word timings.
    pub result: stable_ts::WhisperResult,
}

impl Transcription {
    /// Joined transcript text across all segments (the `.text` attribute).
    #[must_use]
    pub fn text(&self) -> String {
        self.result.text()
    }

    /// Segment-level `{start, end, text}` views, for parity comparison or
    /// downstream consumers that only need the timed lines.
    #[must_use]
    pub fn segments(&self) -> Vec<TranscriptionSegment> {
        self.result
            .segments
            .iter()
            .map(|s| TranscriptionSegment { start: s.start(), end: s.end(), text: s.text() })
            .collect()
    }

    /// Render SRT (`vtt=false`) or VTT (`vtt=true`) at segment level, matching
    /// `TranscriptionResult.to_srt_vtt(word_level=False)`.
    #[must_use]
    pub fn to_srt_vtt(&self, vtt: bool) -> String {
        stable_ts::output::to_srt_vtt(&self.result, false, vtt)
    }

    /// Render segment-level ASS, matching
    /// `TranscriptionResult.to_ass(segment_level=True, word_level=False)`.
    #[must_use]
    pub fn to_ass(&self) -> String {
        stable_ts::output::to_ass(&self.result, false)
    }

    /// Serialize the full result as a compact JSON string, matching
    /// `OutputFormat.JSON` (`json.dumps(result.to_dict())`).
    #[must_use]
    pub fn to_json(&self) -> String {
        stable_ts::output::to_json(&self.result)
    }

    /// The plain-text transcript (no timestamps), matching `OutputFormat.TXT`.
    #[must_use]
    pub fn to_txt(&self) -> String {
        stable_ts::output::to_txt(&self.result)
    }
}

/// A segment-level timed line of the finished transcript.
#[derive(Debug, Clone, PartialEq)]
pub struct TranscriptionSegment {
    /// Segment start, in seconds.
    pub start: f64,
    /// Segment end, in seconds.
    pub end: f64,
    /// Segment text.
    pub text: String,
}

/// Convert raw whisper.cpp inference output into the `to_dict()`-shaped JSON
/// the ported [`stable_ts::WhisperResult`] parses.
///
/// `WhisperResult::from_value` reads top-level `language` (via `ori_dict`) and a
/// `segments` array of `{start, end, text, words: [{word, start, end,
/// probability}]}`, exactly the fields whisper.cpp gives us. We emit that shape
/// so the downstream stages operate on real word timings.
fn raw_to_value(raw: &WhisperResult) -> serde_json::Value {
    use serde_json::{json, Value};

    let segments: Vec<Value> = raw
        .segments
        .iter()
        .map(|seg| {
            let words: Vec<Value> = seg
                .words
                .iter()
                .map(|w| {
                    json!({
                        "word": w.word,
                        "start": w.start,
                        "end": w.end,
                        "probability": w.probability,
                    })
                })
                .collect();
            json!({
                "start": seg.start,
                "end": seg.end,
                "text": seg.text,
                "words": words,
            })
        })
        .collect();

    json!({
        "language": raw.language,
        "segments": segments,
    })
}

/// Run the post-inference stages `WhisperModelWrapper.transcribe` runs after
/// the model decode: regroup the raw result, suppress silence against the
/// decoded PCM, then return a [`Transcription`] ready to render SRT/VTT.
///
/// This is the structural core of the pipeline and is independent of the
/// `model` feature, so the parity falsifier can drive it from a captured raw
/// transcription fixture without a model on hand.
///
/// * `regroup_algo` — the regroup DSL string (e.g. [`DEFAULT_REGROUP`]); an
///   empty string skips regrouping, matching `custom_regroup=False`.
/// * `pcm` — the mono 16 kHz f32 samples the result was decoded from, used to
///   derive the non-VAD silence ranges. Empty (or too-short) audio yields no
///   silence and leaves timings untouched, matching `audio2timings` returning
///   `None`.
pub fn assemble_result(
    raw: &WhisperResult,
    regroup_algo: &str,
    pcm: &[f32],
) -> Result<Transcription, PipelineError> {
    let mut result = stable_ts::WhisperResult::from_value(&raw_to_value(raw));

    // Stage B: regroup. `parse_regroup_algo("")` is an empty op list, so an
    // empty `regroup_algo` is a no-op (custom_regroup disabled).
    let ops = stable_ts::parse_regroup_algo(regroup_algo)
        .map_err(|e| PipelineError::UnknownRegroupMethod(e.0))?;
    stable_ts::apply_regroup(&mut result, &ops)
        .map_err(|e| PipelineError::UnsupportedRegroupMethod(e.0))?;

    // Stage C: suppress silence (non-VAD), the submate default. Derive the
    // silence ranges from the same PCM the model decoded; `None` (silent /
    // too-short audio) means nothing to suppress.
    if let Some((starts, ends)) = stable_ts::audio2timings(pcm) {
        stable_ts::suppress_silence(
            &mut result,
            &starts,
            &ends,
            DEFAULT_MIN_WORD_DUR,
            DEFAULT_NONSPEECH_ERROR,
        );
        stable_ts::update_nonspeech_sections(&mut result, &starts, &ends);
        stable_ts::set_current_as_orig(&mut result);
    }

    let language = result.language.clone();
    Ok(Transcription { language, result })
}

/// End-to-end transcription entry point, mirroring
/// `WhisperModelWrapper.transcribe` (`submate/whisper.py`).
///
/// Media path → PCM (via `submate-media`) → whisper.cpp inference → regroup →
/// suppress-silence → [`Transcription`] (`.text`/`.language`/`.segments`/
/// `.to_srt_vtt`). Real model execution is gated behind the `model` cargo
/// feature; the assembly stages ([`assemble_result`]) are not, so the default
/// build still compiles and the structural parity test runs without a model.
#[cfg(feature = "model")]
pub async fn transcribe(
    model_path: impl Into<String>,
    media_path: &std::path::Path,
    regroup_algo: &str,
    options: TranscribeOptions,
) -> Result<Transcription, TranscribeError> {
    use submate_media::{prepare_audio_for_transcription, PreparedAudio};

    // Prepare audio: extract a track to PCM only when the file has several,
    // otherwise hand whisper the file path directly (matches the Python helper).
    let prepared =
        prepare_audio_for_transcription(media_path, options.language.as_deref()).await;
    let pcm = match prepared {
        PreparedAudio::Pcm(bytes) => pcm_s16le_to_f32(&bytes),
        PreparedAudio::Path(path) => {
            // whisper.cpp needs decoded f32 PCM; decode the whole file's first
            // audio track to s16le and convert.
            let bytes = submate_media::extract_audio_track_to_memory(&path, 0)
                .await
                .map_err(|e| TranscribeError::Media(e.to_string()))?;
            pcm_s16le_to_f32(&bytes)
        }
    };

    let raw = transcribe_pcm(model_path, pcm.clone(), options)
        .await
        .map_err(TranscribeError::Whisper)?;

    assemble_result(&raw, regroup_algo, &pcm).map_err(TranscribeError::Pipeline)
}

/// Errors raised by the end-to-end [`transcribe`] entry point.
#[cfg(feature = "model")]
#[derive(Debug, thiserror::Error)]
pub enum TranscribeError {
    /// Audio extraction / probing failed.
    #[error("media error: {0}")]
    Media(String),
    /// whisper.cpp inference failed.
    #[error("whisper error: {0}")]
    Whisper(#[source] WhisperError),
    /// A post-inference assembly stage failed.
    #[error("pipeline error: {0}")]
    Pipeline(#[source] PipelineError),
}

/// Decode signed 16-bit little-endian PCM bytes into normalized f32 samples in
/// `-1.0..=1.0`, the layout whisper.cpp expects. Mirrors what
/// `extract_audio_track_to_memory` (`s16le`/mono/16 kHz) produces. Any trailing
/// odd byte is ignored.
#[cfg(feature = "model")]
fn pcm_s16le_to_f32(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(2)
        .map(|c| i16::from_le_bytes([c[0], c[1]]) as f32 / 32768.0)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn word_count_sums_segments() {
        let result = WhisperResult {
            language: "en".into(),
            text: "hi there".into(),
            segments: vec![WhisperSegment {
                text: "hi there".into(),
                start: 0.0,
                end: 1.0,
                words: vec![
                    WhisperWord {
                        word: "hi".into(),
                        start: 0.0,
                        end: 0.4,
                        probability: 0.9,
                    },
                    WhisperWord {
                        word: " there".into(),
                        start: 0.4,
                        end: 1.0,
                        probability: 0.8,
                    },
                ],
            }],
        };
        assert_eq!(result.word_count(), 2);
    }

    /// Model-gated smoke test.
    ///
    /// Runs real whisper.cpp inference on the captured PCM clip and asserts the
    /// result is non-empty with per-word timings. Skipped unless built with the
    /// `model` feature and pointed at a model via `SUBMATE_WHISPER_MODEL`, since
    /// neither the model nor the fixture ship with the repo.
    #[cfg(feature = "model")]
    #[tokio::test]
    async fn transcribe_smoke() {
        let model_path = match std::env::var("SUBMATE_WHISPER_MODEL") {
            Ok(p) => p,
            Err(_) => {
                eprintln!("skipping transcribe_smoke: set SUBMATE_WHISPER_MODEL");
                return;
            }
        };

        let fixture = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/stablets/clipA/audio.f32"
        );
        let bytes = match std::fs::read(fixture) {
            Ok(b) => b,
            Err(_) => {
                eprintln!("skipping transcribe_smoke: fixture {fixture} missing");
                return;
            }
        };
        let pcm: Vec<f32> = bytes
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();

        let result = transcribe_pcm(model_path, pcm, TranscribeOptions::default())
            .await
            .expect("transcription should succeed");

        assert!(!result.text.is_empty(), "transcript text should be non-empty");
        assert!(!result.segments.is_empty(), "result should have segments");
        assert!(result.word_count() > 0, "result should have per-word timings");
        for seg in &result.segments {
            for word in &seg.words {
                assert!(word.end >= word.start, "word end must not precede start");
            }
        }
    }
}
