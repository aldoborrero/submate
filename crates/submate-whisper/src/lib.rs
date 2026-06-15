//! whisper-rs transcription pipeline.
//!
//! Wraps native whisper.cpp inference (via `whisper-rs`) and assembles a
//! stable-ts-shaped [`WhisperResult`] carrying per-word timestamps, which the
//! stable-ts slice (regroup / suppress_silence / output) consumes.
//!
//! Real model execution is gated behind the `model` cargo feature. That keeps
//! whisper.cpp (which needs `LIBCLANG_PATH`/`cmake` from the devshell) out of
//! the default build, so CI without a model still compiles the crate and runs
//! the non-model tests.

use std::sync::Arc;
use std::sync::Once;
use std::sync::atomic::{AtomicUsize, Ordering};

use serde::{Deserialize, Serialize};
use tokio::sync::Semaphore;

/// A single recognized word and the time span it occupies, in seconds.
///
/// Per-word timing attached to each segment: `word`, `start`, `end`, plus the
/// model's average token probability for the word.
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
    /// Prompt text biasing the decoder's vocabulary/spelling.
    pub initial_prompt: Option<String>,
    /// Beam-search width; `None` uses greedy decoding.
    pub beam_size: Option<u32>,
    /// Sampling temperature.
    pub temperature: Option<f32>,
    /// No-speech probability above which a segment is treated as silence.
    pub no_speech_threshold: Option<f32>,
    /// Entropy threshold for the decoder's temperature fallback.
    pub entropy_threshold: Option<f32>,
    /// Average-log-probability threshold below which a decode is rejected.
    pub logprob_threshold: Option<f32>,
    /// Maximum characters per segment (caps subtitle line length).
    pub max_len: Option<u32>,
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

/// Caps concurrent transcriptions to a fixed runner count.
///
/// A `Semaphore` gates every transcription behind a permit, so at most
/// `runners` clips transcribe at once and the rest wait. Shared by the
/// in-process CLI (`submate transcribe`) and the Bazarr server path. Clone is
/// cheap: every clone shares the same underlying semaphore.
#[derive(Clone)]
pub struct Dispatcher {
    semaphore: Arc<Semaphore>,
    runners: usize,
}

impl Dispatcher {
    /// Build a dispatcher that allows `runners` transcriptions to run at once.
    ///
    /// # Panics
    ///
    /// Panics if `runners` is zero — nothing could ever make progress, so it is
    /// a configuration error rather than a runtime state.
    pub fn new(runners: usize) -> Self {
        assert!(runners > 0, "a dispatcher must have at least one runner");
        Self {
            semaphore: Arc::new(Semaphore::new(runners)),
            runners,
        }
    }

    /// The configured runner count (the concurrency ceiling).
    pub fn runners(&self) -> usize {
        self.runners
    }

    /// Permits currently available — how many more transcriptions could start
    /// right now without waiting.
    pub fn available_permits(&self) -> usize {
        self.semaphore.available_permits()
    }

    /// Run a blocking transcription step under a runner permit.
    ///
    /// Acquires a permit (waiting if all `runners` are busy), then runs `job`
    /// on a blocking thread via [`tokio::task::spawn_blocking`]. The permit is
    /// held for the entire duration of `job`, so the cap covers the actual work.
    ///
    /// `job` is the injectable blocking step: real callers invoke whisper.cpp
    /// inference; tests pass a closure that blocks on a barrier and bumps a
    /// counter to observe the cap.
    pub async fn transcribe_with<F>(&self, job: F) -> Result<WhisperResult, WhisperError>
    where
        F: FnOnce() -> Result<WhisperResult, WhisperError> + Send + 'static,
    {
        // Holding the owned permit alive until the blocking task finishes keeps
        // the slot reserved for the whole transcription.
        let permit = Arc::clone(&self.semaphore)
            .acquire_owned()
            .await
            .expect("dispatcher semaphore is never closed");

        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            job()
        })
        .await
        .map_err(|e| WhisperError::Join(e.to_string()))?
    }

    /// Transcribe a PCM clip through [`transcribe_pcm`] under a runner permit.
    ///
    /// Available only with the `model` feature, which pulls in whisper.cpp. The
    /// permit is held across the whole inference call so concurrency stays
    /// capped at the runner count.
    #[cfg(feature = "model")]
    pub async fn transcribe_pcm(
        &self,
        model_path: impl Into<String>,
        pcm: Vec<f32>,
        options: TranscribeOptions,
    ) -> Result<WhisperResult, WhisperError> {
        install_whisper_logging();
        let model_path = model_path.into();
        let _permit = self
            .semaphore
            .acquire()
            .await
            .expect("dispatcher semaphore is never closed");
        transcribe_pcm(model_path, pcm, options).await
    }
}

static WHISPER_LOG_HOOK: Once = Once::new();
static WHISPER_LOG_HOOK_INSTALLS: AtomicUsize = AtomicUsize::new(0);

/// Redirect whisper.cpp's process-global stderr logging through `tracing`.
///
/// whisper.cpp installs a process-global log callback, so this must run exactly
/// once; the [`Once`] makes repeated calls a no-op. With the redirection in
/// place the C library's chatter becomes `tracing` events — hidden at the
/// default `INFO` level — so a normal transcribe run no longer floods the
/// terminal. Defined regardless of the `model` feature so the install path
/// stays testable without linking whisper.cpp.
pub fn install_whisper_logging() {
    WHISPER_LOG_HOOK.call_once(|| {
        #[cfg(feature = "model")]
        whisper_rs::install_logging_hooks();
        WHISPER_LOG_HOOK_INSTALLS.fetch_add(1, Ordering::SeqCst);
    });
}

/// How many times [`install_whisper_logging`] has installed the redirection
/// (`0` before the first call, `1` forever after).
pub fn whisper_logging_install_count() -> usize {
    WHISPER_LOG_HOOK_INSTALLS.load(Ordering::SeqCst)
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
    use super::{RawToken, group_tokens_into_words};

    fn tok(bytes: &[u8], t0: i64, t1: i64) -> RawToken {
        RawToken {
            bytes: bytes.to_vec(),
            t0,
            t1,
            prob: 1.0,
        }
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

/// A speech region from VAD, mapping the *filtered* (speech-only) timeline back
/// to the *original* clip timeline. All values in seconds.
#[cfg(any(feature = "model", test))]
#[derive(Debug, Clone, Copy)]
struct VadRegion {
    /// Where this region begins in the concatenated speech-only audio.
    filtered_start: f64,
    /// Where it began in the original clip.
    orig_start: f64,
    /// Region duration.
    dur: f64,
}

/// Map a timestamp on the filtered (speech-only) timeline back to the original
/// clip timeline.
///
/// The filtered audio is the speech regions concatenated, so the map is
/// piecewise: find the region the timestamp falls in and shift it by that
/// region's original offset. A timestamp on a region boundary belongs to the
/// later region; one past the last region clamps to its end.
#[cfg(any(feature = "model", test))]
fn remap_seconds(regions: &[VadRegion], filtered_s: f64) -> f64 {
    for r in regions {
        if filtered_s < r.filtered_start + r.dur {
            return r.orig_start + (filtered_s - r.filtered_start).max(0.0);
        }
    }
    regions.last().map_or(filtered_s, |r| r.orig_start + r.dur)
}

/// Assemble speech-only audio from VAD segment bounds (centiseconds), returning
/// the concatenated PCM and the map back to the original timeline.
///
/// This reproduces what whisper.cpp's `whisper_vad()` does before transcription
/// — the step whisper-rs's `state.full` bypasses: each **non-final** segment is
/// extended by `overlap_samples` so a word straddling the boundary isn't
/// clipped, and `silence_samples` of silence separate the kept regions so the
/// model doesn't run adjacent speech together. Degenerate (empty) segments are
/// dropped. Each region copies a contiguous original slice, so the timeline map
/// is slope-1 within a region (see [`remap_seconds`]).
#[cfg(any(feature = "model", test))]
fn assemble_speech_only(
    pcm: &[f32],
    segments_cs: &[(f32, f32)],
    overlap_samples: usize,
    silence_samples: usize,
) -> (Vec<f32>, Vec<VadRegion>) {
    let per_cs = f64::from(SAMPLE_RATE / 100);
    let sr = f64::from(SAMPLE_RATE);
    let n = segments_cs.len();
    let mut filtered: Vec<f32> = Vec::new();
    let mut regions: Vec<VadRegion> = Vec::new();
    for (i, &(start_cs, end_cs)) in segments_cs.iter().enumerate() {
        let s0 = ((f64::from(start_cs) * per_cs) as usize).min(pcm.len());
        let mut s1 = (f64::from(end_cs) * per_cs) as usize;
        // Extend every non-final segment into the next one so the boundary word
        // isn't cut off; clamp to the clip end.
        if i + 1 < n {
            s1 += overlap_samples;
        }
        let s1 = s1.min(pcm.len());
        if s1 <= s0 {
            continue;
        }
        // Separate kept regions with silence (never before the first kept one).
        if !regions.is_empty() {
            filtered.resize(filtered.len() + silence_samples, 0.0);
        }
        let filtered_start = filtered.len() as f64 / sr;
        filtered.extend_from_slice(&pcm[s0..s1]);
        regions.push(VadRegion {
            filtered_start,
            orig_start: f64::from(start_cs) / 100.0,
            dur: (s1 - s0) as f64 / sr,
        });
    }
    (filtered, regions)
}

#[cfg(test)]
mod vad_remap_tests {
    use super::{VadRegion, remap_seconds};

    // Original speech at [10,15) and [40,45), concatenated to filtered [0,5) and
    // [5,10) — the 25s silence between them is dropped.
    fn regions() -> Vec<VadRegion> {
        vec![
            VadRegion {
                filtered_start: 0.0,
                orig_start: 10.0,
                dur: 5.0,
            },
            VadRegion {
                filtered_start: 5.0,
                orig_start: 40.0,
                dur: 5.0,
            },
        ]
    }

    #[test]
    fn maps_filtered_time_back_to_original() {
        let r = regions();
        assert_eq!(remap_seconds(&r, 0.0), 10.0); // region 1 start
        assert_eq!(remap_seconds(&r, 2.5), 12.5); // mid region 1
        assert_eq!(remap_seconds(&r, 5.0), 40.0); // boundary -> region 2 start
        assert_eq!(remap_seconds(&r, 7.5), 42.5); // mid region 2
    }

    #[test]
    fn clamps_past_the_last_region() {
        assert_eq!(remap_seconds(&regions(), 99.0), 45.0);
    }

    #[test]
    fn empty_regions_pass_through() {
        assert_eq!(remap_seconds(&[], 3.0), 3.0);
    }
}

#[cfg(test)]
mod vad_assemble_tests {
    use super::{SAMPLE_RATE, assemble_speech_only, remap_seconds};

    // 16 kHz: 160 samples per centisecond, 16000 per second.
    #[test]
    fn extends_overlap_pads_silence_and_remaps() {
        let sr = SAMPLE_RATE as usize;
        let pcm = vec![1.0f32; sr * 4]; // 4 s of (nonzero) audio
        // Two speech segments, centiseconds: [0, 1 s) and [2 s, 3 s).
        let segments = [(0.0f32, 100.0f32), (200.0f32, 300.0f32)];
        let overlap = sr / 10; // 0.1 s
        let silence = sr / 10; // 0.1 s

        let (filtered, regions) = assemble_speech_only(&pcm, &segments, overlap, silence);

        // seg0 (non-final) extended by overlap, then a silence gap, then seg0 as-is.
        assert_eq!(filtered.len(), (sr + overlap) + silence + sr);
        assert_eq!(regions.len(), 2);

        // The inserted gap is the only silence in an otherwise-1.0 buffer.
        let gap = sr + overlap;
        assert!(filtered[..gap].iter().all(|&s| s == 1.0));
        assert!(filtered[gap..gap + silence].iter().all(|&s| s == 0.0));
        assert!(filtered[gap + silence..].iter().all(|&s| s == 1.0));

        // Region 0 carries the 0.1 s overlap; region 1 starts after the gap.
        assert_eq!(regions[0].orig_start, 0.0);
        assert_eq!(regions[1].orig_start, 2.0);
        assert!((regions[0].dur - 1.1).abs() < 1e-9);
        assert!((regions[1].filtered_start - 1.2).abs() < 1e-9);

        // Timestamps map back to the original timeline; a point inside the
        // inserted gap snaps to the next region's original start.
        assert!((remap_seconds(&regions, 0.5) - 0.5).abs() < 1e-9); // mid region 0
        assert!((remap_seconds(&regions, 1.15) - 2.0).abs() < 1e-9); // in the gap
        assert!((remap_seconds(&regions, 1.7) - 2.5).abs() < 1e-9); // mid region 1
    }

    #[test]
    fn single_segment_gets_no_overlap_or_silence() {
        let sr = SAMPLE_RATE as usize;
        let pcm = vec![1.0f32; sr * 2];
        let (filtered, regions) = assemble_speech_only(&pcm, &[(0.0, 100.0)], sr / 10, sr / 10);
        assert_eq!(regions.len(), 1);
        assert_eq!(filtered.len(), sr); // exactly [0, 1 s), no overlap, no trailing silence
        assert!((regions[0].dur - 1.0).abs() < 1e-9);
    }
}

#[cfg(feature = "model")]
mod inference {
    use super::*;
    use whisper_rs::{
        FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, WhisperVadContext,
        WhisperVadContextParams, WhisperVadParams,
    };

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

    /// Optional whisper.cpp thread-count override from `SUBMATE__WHISPER__THREADS`.
    ///
    /// Returns `None` (leave whisper.cpp's own default of `min(4, n_cpu)`) unless
    /// the env var is set. Measured on a 20-thread box with the `base` model,
    /// raising the thread count above the default *regresses* (4→27s, 8→37s,
    /// 20→113s): inference is memory-bandwidth-bound, so oversubscription
    /// thrashes. The optimum is model- and host-dependent (a large model on many
    /// physical cores may benefit), so we expose it as a knob instead of forcing
    /// a value that helps in theory but hurts in practice.
    fn whisper_threads() -> Option<std::os::raw::c_int> {
        std::env::var("SUBMATE__WHISPER__THREADS")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .map(clamp_threads)
    }

    /// Path to a Silero VAD model from `SUBMATE__WHISPER__VAD_MODEL`, or `None` to
    /// leave VAD off. Present-and-non-empty turns on speech-only transcription.
    fn whisper_vad_model() -> Option<String> {
        std::env::var("SUBMATE__WHISPER__VAD_MODEL")
            .ok()
            .filter(|s| !s.is_empty())
    }

    /// Speech-segment overlap and inter-segment silence, both 0.1 s — matching
    /// whisper.cpp's `whisper_vad()` defaults (`samples_overlap` and its fixed
    /// 0.1 s gap). See [`assemble_speech_only`].
    const VAD_OVERLAP_S: f64 = 0.1;
    const VAD_SILENCE_S: f64 = 0.1;

    /// Run Silero VAD over `pcm`, returning the speech-only PCM (the detected
    /// speech regions concatenated) and the map back to the original timeline.
    ///
    /// whisper-rs's `state.full` calls `whisper_full_with_state`, which skips
    /// whisper.cpp's built-in VAD (that lives only in the `whisper_full`
    /// wrapper), so we drive the VAD engine ourselves and reproduce the same
    /// assembly the wrapper does — see [`assemble_speech_only`].
    fn run_vad(vad_model: &str, pcm: &[f32]) -> Result<(Vec<f32>, Vec<VadRegion>), WhisperError> {
        let mut vctx = WhisperVadContext::new(vad_model, WhisperVadContextParams::default())
            .map_err(|e| WhisperError::Load(e.to_string()))?;
        let segments = vctx
            .segments_from_samples(WhisperVadParams::default(), pcm)
            .map_err(|e| WhisperError::Inference(e.to_string()))?;

        // Collect the VAD segment bounds (centiseconds), then assemble the
        // speech-only audio exactly as whisper.cpp's own VAD path would.
        let mut bounds: Vec<(f32, f32)> = Vec::new();
        for i in 0..segments.num_segments() {
            if let (Some(start_cs), Some(end_cs)) = (
                segments.get_segment_start_timestamp(i),
                segments.get_segment_end_timestamp(i),
            ) {
                bounds.push((start_cs, end_cs));
            }
        }

        let sr = f64::from(SAMPLE_RATE);
        let (filtered, regions) = assemble_speech_only(
            pcm,
            &bounds,
            (VAD_OVERLAP_S * sr) as usize,
            (VAD_SILENCE_S * sr) as usize,
        );
        tracing::debug!(
            speech_regions = regions.len(),
            speech_secs = filtered.len() as f64 / sr,
            clip_secs = pcm.len() as f64 / sr,
            "VAD kept speech-only audio"
        );
        Ok((filtered, regions))
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

        // Beam search when a width is given, else greedy (whisper.cpp's default).
        let strategy = match options.beam_size {
            Some(n) => SamplingStrategy::BeamSearch {
                beam_size: n as i32,
                patience: -1.0,
            },
            None => SamplingStrategy::Greedy { best_of: 1 },
        };
        let mut params = FullParams::new(strategy);
        // Only override whisper.cpp's default thread count when explicitly asked
        // (SUBMATE__WHISPER__THREADS) — forcing more threads regresses small models.
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
        // Optional decoding knobs (CLI flags / SUBMATE__WHISPER__*); each leaves
        // whisper.cpp's own default in place when unset.
        if let Some(prompt) = options.initial_prompt.as_deref() {
            params.set_initial_prompt(prompt);
        }
        if let Some(t) = options.temperature {
            params.set_temperature(t);
        }
        if let Some(t) = options.no_speech_threshold {
            params.set_no_speech_thold(t);
        }
        if let Some(t) = options.entropy_threshold {
            params.set_entropy_thold(t);
        }
        if let Some(t) = options.logprob_threshold {
            params.set_logprob_thold(t);
        }
        if let Some(n) = options.max_len {
            params.set_max_len(n as i32);
        }
        // Quiet: whisper.cpp's stdout callbacks have no place in a library.
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);

        // VAD: when SUBMATE__WHISPER__VAD_MODEL is set, transcribe only the detected
        // speech and map timings back below; a VAD miss (no speech) falls back to
        // the full clip so audio is never dropped.
        let vad = match whisper_vad_model().as_deref() {
            Some(model) => {
                let (filtered, regions) = run_vad(model, pcm)?;
                (!regions.is_empty()).then_some((filtered, regions))
            }
            None => None,
        };
        let (samples, vad_regions) = match &vad {
            Some((filtered, regions)) => (filtered.as_slice(), Some(regions.as_slice())),
            None => (pcm, None),
        };

        state
            .full(params, samples)
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

        // Shift speech-only timings back onto the original clip timeline.
        if let Some(regions) = vad_regions {
            for seg in &mut segments {
                seg.start = remap_seconds(regions, seg.start);
                seg.end = remap_seconds(regions, seg.end);
                for word in &mut seg.words {
                    word.start = remap_seconds(regions, word.start);
                    word.end = remap_seconds(regions, word.end);
                }
            }
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

/// The finished transcription the CLI and queue consume: `.text`,
/// `.language`, `.segments`, and
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
            .map(|s| TranscriptionSegment {
                start: s.start(),
                end: s.end(),
                text: s.text(),
            })
            .collect()
    }

    /// Render SRT (`vtt=false`) or VTT (`vtt=true`). `word_level` emits one block
    /// per word (karaoke-style) instead of per segment.
    #[must_use]
    pub fn to_srt_vtt(&self, word_level: bool, vtt: bool) -> String {
        stable_ts::output::to_srt_vtt(&self.result, word_level, vtt)
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
    use serde_json::{Value, json};

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

/// Post-inference assembly knobs (the `[stable_ts]` config). [`Default`]
/// reproduces the historical hardcoded behavior ([`DEFAULT_REGROUP`],
/// suppress-silence on, [`DEFAULT_MIN_WORD_DUR`]).
#[derive(Debug, Clone)]
pub struct AssembleOptions {
    /// Regroup DSL string; an empty string disables regrouping.
    pub regroup_algo: String,
    /// Whether to run the suppress-silence stage.
    pub suppress_silence: bool,
    /// `min_word_dur` for the suppress-silence stage.
    pub min_word_duration: f64,
}

impl Default for AssembleOptions {
    fn default() -> Self {
        Self {
            regroup_algo: DEFAULT_REGROUP.to_string(),
            suppress_silence: true,
            min_word_duration: DEFAULT_MIN_WORD_DUR,
        }
    }
}

/// Run the post-inference stages after the model decode: regroup the raw
/// result, optionally suppress silence against the decoded PCM, then return a
/// [`Transcription`] ready to render SRT/VTT.
///
/// This is the structural core of the pipeline and is independent of the
/// `model` feature, so the parity falsifier can drive it from a captured raw
/// transcription fixture without a model on hand.
///
/// * `opts` — the `[stable_ts]` knobs (regroup string, suppress on/off,
///   min word duration). See [`AssembleOptions`].
/// * `pcm` — the mono 16 kHz f32 samples the result was decoded from, used to
///   derive the non-VAD silence ranges. Empty (or too-short) audio yields no
///   silence and leaves timings untouched.
pub fn assemble_result(
    raw: &WhisperResult,
    opts: &AssembleOptions,
    pcm: &[f32],
) -> Result<Transcription, PipelineError> {
    let mut result = stable_ts::WhisperResult::from_value(&raw_to_value(raw));

    // Regroup. `parse_regroup_algo("")` is an empty op list, so an empty
    // `regroup_algo` is a no-op (custom_regroup disabled).
    let ops = stable_ts::parse_regroup_algo(&opts.regroup_algo)
        .map_err(|e| PipelineError::UnknownRegroupMethod(e.0))?;
    stable_ts::apply_regroup(&mut result, &ops)
        .map_err(|e| PipelineError::UnsupportedRegroupMethod(e.0))?;

    // Suppress silence (non-VAD), when enabled. Derive the silence ranges from
    // the same PCM the model decoded; `None` (silent / too-short audio) means
    // nothing to suppress.
    if opts.suppress_silence
        && let Some((starts, ends)) = stable_ts::audio2timings(pcm)
    {
        stable_ts::suppress_silence(
            &mut result,
            &starts,
            &ends,
            opts.min_word_duration,
            DEFAULT_NONSPEECH_ERROR,
        );
        stable_ts::update_nonspeech_sections(&mut result, &starts, &ends);
        stable_ts::set_current_as_orig(&mut result);
    }

    let language = result.language.clone();
    Ok(Transcription { language, result })
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

        assert!(
            !result.text.is_empty(),
            "transcript text should be non-empty"
        );
        assert!(!result.segments.is_empty(), "result should have segments");
        assert!(
            result.word_count() > 0,
            "result should have per-word timings"
        );
        for seg in &result.segments {
            for word in &seg.words {
                assert!(word.end >= word.start, "word end must not precede start");
            }
        }
    }
}

#[cfg(test)]
mod dispatcher_tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Condvar, Mutex};
    use std::time::Duration;

    use tokio::time::timeout;

    fn lang_result(language: &str) -> WhisperResult {
        WhisperResult {
            language: language.to_string(),
            text: String::new(),
            segments: Vec::new(),
        }
    }

    /// A gate the blocking jobs park on synchronously (they run off the async
    /// runtime, so they use std primitives, not tokio ones). The test opens the
    /// gate once it has confirmed the third job is still waiting for a permit.
    #[derive(Default)]
    struct Gate {
        open: Mutex<bool>,
        cv: Condvar,
    }

    impl Gate {
        fn wait(&self) {
            let mut open = self.open.lock().unwrap();
            while !*open {
                open = self.cv.wait(open).unwrap();
            }
        }

        fn release(&self) {
            *self.open.lock().unwrap() = true;
            self.cv.notify_all();
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn dispatcher_caps_concurrency() {
        let dispatcher = Dispatcher::new(2);

        // Counters observe how many jobs are inside the blocking step at once.
        let active = Arc::new(AtomicUsize::new(0));
        let max_active = Arc::new(AtomicUsize::new(0));
        let started = Arc::new(AtomicUsize::new(0));
        let gate = Arc::new(Gate::default());

        let spawn = |id: usize| {
            let dispatcher = dispatcher.clone();
            let active = Arc::clone(&active);
            let max_active = Arc::clone(&max_active);
            let started = Arc::clone(&started);
            let gate = Arc::clone(&gate);
            tokio::spawn(async move {
                dispatcher
                    .transcribe_with(move || {
                        started.fetch_add(1, Ordering::SeqCst);
                        let now = active.fetch_add(1, Ordering::SeqCst) + 1;
                        max_active.fetch_max(now, Ordering::SeqCst);
                        // Park inside the blocking step (and thus while holding a
                        // permit) until the test opens the gate.
                        gate.wait();
                        active.fetch_sub(1, Ordering::SeqCst);
                        Ok(lang_result(&format!("lang{id}")))
                    })
                    .await
            })
        };

        let h1 = spawn(1);
        let h2 = spawn(2);
        let h3 = spawn(3);

        // Wait until two jobs are parked holding permits, then confirm the third
        // is still blocked: only `runners` (2) permits exist, so exactly two can
        // be inside the blocking step. If the cap leaked, all three would start.
        let two_running = timeout(Duration::from_secs(5), async {
            loop {
                if started.load(Ordering::SeqCst) >= 2 {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await;
        assert!(two_running.is_ok(), "first two jobs never both started");

        // Give a leaked third job a chance to also start before we assert.
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(
            started.load(Ordering::SeqCst),
            2,
            "a third job ran while both permits were held — concurrency cap leaked"
        );
        assert_eq!(dispatcher.available_permits(), 0);

        // Release every parked job. As the first two drain they free permits and
        // the third finally acquires one and runs.
        gate.release();
        let results = timeout(Duration::from_secs(5), async {
            let r1 = h1.await.expect("task 1 panicked");
            let r2 = h2.await.expect("task 2 panicked");
            let r3 = h3.await.expect("task 3 panicked");
            (r1, r2, r3)
        })
        .await
        .expect("dispatcher deadlocked or starved a permit");

        // Results return correctly for all three submissions.
        let (r1, r2, r3) = results;
        let langs: Vec<String> = [r1, r2, r3]
            .into_iter()
            .map(|r| r.expect("transcription failed").language)
            .collect();
        for want in ["lang1", "lang2", "lang3"] {
            assert!(langs.contains(&want.to_string()), "missing result {want}");
        }

        // Never more than `runners` jobs ran the blocking step at once.
        assert!(
            max_active.load(Ordering::SeqCst) <= 2,
            "concurrency exceeded the runner cap: saw {} active",
            max_active.load(Ordering::SeqCst)
        );
        // Permits are all returned after the work drains.
        assert_eq!(dispatcher.available_permits(), 2);
    }

    #[tokio::test]
    async fn runners_reports_configured_count() {
        let dispatcher = Dispatcher::new(3);
        assert_eq!(dispatcher.runners(), 3);
        assert_eq!(dispatcher.available_permits(), 3);
    }

    #[tokio::test]
    async fn errors_propagate_and_release_permit() {
        let dispatcher = Dispatcher::new(1);
        let result = dispatcher
            .transcribe_with(|| Err(WhisperError::Inference("boom".into())))
            .await;
        assert!(matches!(result, Err(WhisperError::Inference(_))));
        // The permit is returned even when the job errors.
        assert_eq!(dispatcher.available_permits(), 1);
    }

    #[tokio::test]
    #[should_panic(expected = "at least one runner")]
    async fn zero_runners_panics() {
        let _ = Dispatcher::new(0);
    }

    /// The whisper.cpp logging redirection installs exactly once, even across
    /// repeated calls — whisper.cpp's log callback is process-global.
    #[test]
    fn whisper_logging_installs_once() {
        install_whisper_logging();
        assert_eq!(whisper_logging_install_count(), 1);
        for _ in 0..5 {
            install_whisper_logging();
        }
        assert_eq!(whisper_logging_install_count(), 1);
    }
}
