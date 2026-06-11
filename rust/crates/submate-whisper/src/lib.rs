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

#[cfg(feature = "model")]
mod inference {
    use super::*;
    use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

    /// whisper.cpp reports token/segment times in centiseconds (1/100 s).
    fn centiseconds_to_seconds(cs: i64) -> f64 {
        cs as f64 / 100.0
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
        let ctx = WhisperContext::new_with_params(model_path, WhisperContextParameters::default())
            .map_err(|e| WhisperError::Load(e.to_string()))?;

        let mut state = ctx
            .create_state()
            .map_err(|e| WhisperError::Load(e.to_string()))?;

        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
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

        let n_segments = state
            .full_n_segments()
            .map_err(|e| WhisperError::Inference(e.to_string()))?;

        let mut segments = Vec::with_capacity(n_segments.max(0) as usize);
        let mut full_text = String::new();

        for seg in 0..n_segments {
            let seg_text = state
                .full_get_segment_text(seg)
                .map_err(|e| WhisperError::Inference(e.to_string()))?;
            let seg_t0 = state
                .full_get_segment_t0(seg)
                .map_err(|e| WhisperError::Inference(e.to_string()))?;
            let seg_t1 = state
                .full_get_segment_t1(seg)
                .map_err(|e| WhisperError::Inference(e.to_string()))?;

            full_text.push_str(&seg_text);

            let words = collect_words(&state, seg)?;
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
    /// whisper.cpp emits sub-word tokens; we start a new word whenever a token's
    /// text begins with a space (whisper's word boundary marker) and accumulate
    /// each word's span and mean probability from its constituent tokens.
    fn collect_words(
        state: &whisper_rs::WhisperState,
        seg: i32,
    ) -> Result<Vec<WhisperWord>, WhisperError> {
        let n_tokens = state
            .full_n_tokens(seg)
            .map_err(|e| WhisperError::Inference(e.to_string()))?;

        let mut words: Vec<WhisperWord> = Vec::new();
        let mut prob_sum = 0.0_f64;
        let mut prob_count = 0_u32;

        for tok in 0..n_tokens {
            let text = state
                .full_get_token_text(seg, tok)
                .map_err(|e| WhisperError::Inference(e.to_string()))?;
            let data = state
                .full_get_token_data(seg, tok)
                .map_err(|e| WhisperError::Inference(e.to_string()))?;

            // Special tokens (e.g. `[_BEG_]`) carry no real timing; skip them.
            if text.starts_with("[_") && text.ends_with(']') {
                continue;
            }

            let start = centiseconds_to_seconds(data.t0);
            let end = centiseconds_to_seconds(data.t1);
            let prob = data.p as f64;

            let begins_word = text.starts_with(' ') || words.is_empty();
            if begins_word {
                finalize_probability(words.last_mut(), prob_sum, prob_count);
                prob_sum = 0.0;
                prob_count = 0;
                words.push(WhisperWord {
                    word: text,
                    start,
                    end,
                    probability: 0.0,
                });
            } else if let Some(last) = words.last_mut() {
                last.word.push_str(&text);
                last.end = end;
            }
            prob_sum += prob;
            prob_count += 1;
        }

        finalize_probability(words.last_mut(), prob_sum, prob_count);

        Ok(words)
    }

    /// Set the mean probability on the word being completed.
    fn finalize_probability(word: Option<&mut WhisperWord>, prob_sum: f64, prob_count: u32) {
        if let Some(last) = word {
            if prob_count > 0 {
                last.probability = prob_sum / prob_count as f64;
            }
        }
    }

    fn detect_language(state: &whisper_rs::WhisperState) -> String {
        state
            .full_lang_id_from_state()
            .ok()
            .and_then(whisper_rs::get_lang_str)
            .unwrap_or("en")
            .to_string()
    }
}

#[cfg(feature = "model")]
pub use inference::transcribe_pcm;

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
