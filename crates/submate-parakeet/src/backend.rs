//! The transcribe.cpp backend implementation and its `Transcript` → `WhisperResult` map.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use submate_whisper::{
    Task, TranscribeOptions, Transcriber, WhisperError, WhisperResult, WhisperSegment, WhisperWord,
};
use transcribe_cpp::{Model, RunOptions, Task as CppTask, TimestampKind, Token, Transcript};

/// Process-wide cache of loaded transcribe.cpp models, keyed by model file path.
///
/// Mirrors the whisper backend's context cache: `Model::load` parses and uploads
/// the entire GGUF (hundreds of MB for Parakeet), so reloading it on every request
/// dominates the cost. A cached `Arc<Model>` is shared across calls; each call
/// makes its own short-lived [`transcribe_cpp::Session`].
///
/// Trade-off worth knowing: transcribe.cpp serializes the compute path *per model*
/// (an internal per-model lock), so concurrent transcriptions on one cached model
/// queue rather than run in parallel — unlike whisper, whose per-call states
/// parallelize. For submate's background subtitle workload "no reloads, serial
/// compute" is the right trade; a per-path model *pool* could restore parallelism
/// if a future workload needs it.
fn model_cache() -> &'static Mutex<HashMap<String, Arc<Model>>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Arc<Model>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Return the cached model for `model_path`, loading and caching it on first use.
///
/// The load holds the cache lock, so a cold-start race serializes on the first
/// load and the losers reuse the freshly cached model — both cheap and correct.
fn cached_model(model_path: &str) -> Result<Arc<Model>, WhisperError> {
    let mut cache = model_cache()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(model) = cache.get(model_path) {
        return Ok(Arc::clone(model));
    }
    tracing::debug!(model = model_path, "loading parakeet model (cache miss)");
    let model = Arc::new(
        Model::load(model_path).map_err(|error| WhisperError::Load(error.to_string()))?,
    );
    cache.insert(model_path.to_string(), Arc::clone(&model));
    Ok(model)
}

/// Transcription backend backed by transcribe.cpp — primarily for Parakeet.
///
/// A zero-sized handle: like [`submate_whisper::WhisperBackend`] it holds no
/// state, resolving the model through a process-wide cache (see [`cached_model`])
/// so a hot model loads only once. transcribe.cpp sessions are `Send` but not
/// `Sync` and mutate through `&mut self`, so each call takes the cached model and
/// spins up its own short-lived session — the `Dispatcher` already hands each
/// transcription its own blocking thread under a permit.
#[derive(Debug, Default, Clone, Copy)]
pub struct ParakeetBackend;

impl Transcriber for ParakeetBackend {
    fn transcribe_blocking(
        &self,
        model_path: &str,
        pcm: &[f32],
        options: &TranscribeOptions,
    ) -> Result<WhisperResult, WhisperError> {
        let model = cached_model(model_path)?;
        let capabilities = model.capabilities();
        // A model that produces no timestamps yields a subtitle with every cue at
        // 0:00 — worse than no output. Reject it up front rather than emit garbage.
        if capabilities.max_timestamp_kind == TimestampKind::None {
            return Err(WhisperError::Unsupported(format!(
                "model at {model_path} produces no timestamps"
            )));
        }
        let mut session = model
            .session()
            .map_err(|error| WhisperError::Load(error.to_string()))?;

        let run = RunOptions {
            // Ask for the finest timestamps the model advertises (word/token for
            // Parakeet). Requesting finer than the model supports is a hard error,
            // so clamping to the advertised max is the safe request.
            timestamps: capabilities.max_timestamp_kind,
            task: match options.task {
                Task::Transcribe => CppTask::Transcribe,
                Task::Translate => CppTask::Translate,
            },
            language: options.language.clone(),
            ..Default::default()
        };

        let transcript = session
            .run(pcm, &run)
            .map_err(|error| WhisperError::Inference(error.to_string()))?;
        Ok(map_transcript(transcript, options))
    }
}

/// Milliseconds → seconds (transcribe.cpp reports every time in i64 ms).
fn secs(ms: i64) -> f64 {
    ms as f64 / 1000.0
}

/// Fold a transcribe.cpp [`Transcript`] into submate's [`WhisperResult`].
///
/// transcribe.cpp returns flat `segments`/`words`/`tokens` arrays cross-linked by
/// index ranges (`Segment::first_word`/`n_words`, `Word::first_token`/`n_tokens`).
/// This rebuilds submate's nested `segment → words` shape, deriving each word's
/// probability as the mean of its tokens' confidences. `words` is empty when the
/// model only produced segment-level timestamps, which collapses cleanly to
/// segments with no per-word timing.
fn map_transcript(transcript: Transcript, options: &TranscribeOptions) -> WhisperResult {
    let Transcript {
        text,
        language,
        segments,
        words,
        tokens,
        ..
    } = transcript;

    let segments = segments
        .iter()
        .map(|segment| {
            let start = segment.first_word.max(0) as usize;
            let count = segment.n_words.max(0) as usize;
            // Clamp rather than range-index: a segment whose declared word count
            // overruns the flat `words` array should still yield the words that do
            // exist, not silently drop the whole segment.
            let words = words
                .get(start..)
                .unwrap_or(&[])
                .iter()
                .take(count)
                .map(|word| WhisperWord {
                    word: word.text.clone(),
                    start: secs(word.t0_ms),
                    end: secs(word.t1_ms),
                    probability: mean_token_prob(&tokens, word.first_token, word.n_tokens),
                })
                .collect();
            WhisperSegment {
                text: segment.text.clone(),
                start: secs(segment.t0_ms),
                end: secs(segment.t1_ms),
                words,
            }
        })
        .collect();

    WhisperResult {
        // Prefer the model's detected language; fall back to the forced hint.
        language: language.or_else(|| options.language.clone()).unwrap_or_default(),
        text,
        segments,
    }
}

/// Mean of a word's tokens' confidence over `tokens[first .. first + n]`, skipping
/// NaNs (transcribe.cpp uses NaN for "no confidence"); `0.0` when none apply.
fn mean_token_prob(tokens: &[Token], first: i32, n: i32) -> f64 {
    let start = first.max(0) as usize;
    let count = n.max(0) as usize;
    let (sum, seen) = tokens
        .get(start..)
        .unwrap_or(&[])
        .iter()
        .take(count)
        .map(|token| token.p as f64)
        .filter(|probability| probability.is_finite())
        .fold((0.0, 0usize), |(sum, seen), probability| {
            (sum + probability, seen + 1)
        });
    if seen == 0 { 0.0 } else { sum / seen as f64 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use transcribe_cpp::{Segment, Word};

    // Pure mapping test: builds a fake Transcript and checks the fold. Exercises
    // the index cross-linking and mean-probability logic without loading a model.
    #[test]
    fn folds_segments_words_and_mean_probability() {
        let transcript = Transcript {
            text: "hello world".to_string(),
            language: Some("en".to_string()),
            segments: vec![Segment {
                t0_ms: 0,
                t1_ms: 2000,
                first_word: 0,
                n_words: 2,
                first_token: 0,
                n_tokens: 3,
                text: "hello world".to_string(),
                ..Default::default()
            }],
            words: vec![
                Word {
                    t0_ms: 0,
                    t1_ms: 900,
                    first_token: 0,
                    n_tokens: 1,
                    text: "hello".to_string(),
                    ..Default::default()
                },
                Word {
                    t0_ms: 1000,
                    t1_ms: 2000,
                    first_token: 1,
                    n_tokens: 2,
                    text: "world".to_string(),
                    ..Default::default()
                },
            ],
            tokens: vec![
                Token { p: 0.8, ..Default::default() },
                Token { p: 0.6, ..Default::default() },
                Token { p: 0.4, ..Default::default() },
            ],
            ..Default::default()
        };

        let result = map_transcript(transcript, &TranscribeOptions::default());

        assert_eq!(result.language, "en");
        assert_eq!(result.text, "hello world");
        assert_eq!(result.segments.len(), 1);

        let segment = &result.segments[0];
        assert_eq!((segment.start, segment.end), (0.0, 2.0));
        assert_eq!(segment.words.len(), 2);

        assert_eq!(segment.words[0].word, "hello");
        assert_eq!((segment.words[0].start, segment.words[0].end), (0.0, 0.9));
        // Tolerance is f32-wide: token confidences are f32, so 0.8 round-trips as
        // ~0.80000001 once widened to f64.
        assert!((segment.words[0].probability - 0.8).abs() < 1e-6);

        // "world" spans tokens 1..3 → mean(0.6, 0.4) = 0.5.
        assert!((segment.words[1].probability - 0.5).abs() < 1e-6);
    }

    #[test]
    fn segment_without_words_maps_to_a_timed_but_word_free_segment() {
        let transcript = Transcript {
            segments: vec![Segment {
                t0_ms: 500,
                t1_ms: 1500,
                first_word: 0,
                n_words: 0,
                text: "[music]".to_string(),
                ..Default::default()
            }],
            ..Default::default()
        };

        let result = map_transcript(transcript, &TranscribeOptions::default());

        let segment = &result.segments[0];
        assert_eq!((segment.start, segment.end), (0.5, 1.5));
        assert!(segment.words.is_empty());
    }

    #[test]
    fn overrunning_word_count_is_clamped_not_dropped() {
        // Segment declares 5 words but only 1 exists; the real word must survive
        // (the old range-index returned None here and dropped the whole segment).
        let transcript = Transcript {
            segments: vec![Segment {
                first_word: 0,
                n_words: 5,
                text: "hi".to_string(),
                ..Default::default()
            }],
            words: vec![Word {
                text: "hi".to_string(),
                ..Default::default()
            }],
            ..Default::default()
        };

        let result = map_transcript(transcript, &TranscribeOptions::default());

        assert_eq!(result.segments[0].words.len(), 1);
        assert_eq!(result.segments[0].words[0].word, "hi");
    }

    #[test]
    fn nan_token_probabilities_are_skipped() {
        let transcript = Transcript {
            segments: vec![Segment {
                first_word: 0,
                n_words: 1,
                text: "x".to_string(),
                ..Default::default()
            }],
            words: vec![Word {
                first_token: 0,
                n_tokens: 2,
                text: "x".to_string(),
                ..Default::default()
            }],
            tokens: vec![
                Token { p: f32::NAN, ..Default::default() },
                Token { p: 0.5, ..Default::default() },
            ],
            ..Default::default()
        };

        let result = map_transcript(transcript, &TranscribeOptions::default());

        // Only the finite 0.5 contributes to the mean.
        assert!((result.segments[0].words[0].probability - 0.5).abs() < 1e-6);
    }

    #[test]
    fn all_nan_probabilities_default_to_zero() {
        let transcript = Transcript {
            segments: vec![Segment {
                first_word: 0,
                n_words: 1,
                ..Default::default()
            }],
            words: vec![Word {
                first_token: 0,
                n_tokens: 1,
                ..Default::default()
            }],
            tokens: vec![Token { p: f32::NAN, ..Default::default() }],
            ..Default::default()
        };

        let result = map_transcript(transcript, &TranscribeOptions::default());

        assert_eq!(result.segments[0].words[0].probability, 0.0);
    }

    #[test]
    fn empty_transcript_maps_to_an_empty_result() {
        let result = map_transcript(Transcript::default(), &TranscribeOptions::default());
        assert!(result.segments.is_empty());
        assert!(result.text.is_empty());
    }

    #[test]
    fn missing_language_falls_back_to_the_forced_hint() {
        let options = TranscribeOptions {
            language: Some("ja".to_string()),
            ..Default::default()
        };
        let result = map_transcript(Transcript::default(), &options);
        assert_eq!(result.language, "ja");
    }
}
