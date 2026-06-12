//! LLM translation backends (ports `submate/translation.py`).
//!
//! This crate ports the backend-agnostic *machinery* of the Python
//! `TranslationService`:
//!
//! * [`Backend`] — the trait ported from `TranslationBackendBase`. Subclasses
//!   in Python implement only `_complete` (client construction + response
//!   extraction); prompt construction lives in the base `translate`. The Rust
//!   trait mirrors that split: implementors provide [`Backend::complete`]
//!   (HTTP, out of scope here), and the provided [`Backend::translate`] does the
//!   shared prompt formatting.
//! * [`chunk_ranges`] / [`join_batch`] / [`split_batch`] — the chunked batch
//!   logic from `translate_subtitles` / `_translate_batch`: split inputs into
//!   `ceil(len / chunk_size)` batches, join each batch with a separator token,
//!   then split the model reply back into stripped blocks, falling back to the
//!   originals when the returned block count does not match the input count.
//!
//! Ollama is the default local backend, ported here as [`OllamaBackend`]. The
//! remaining backend HTTP calls (Claude/OpenAI/Gemini) are out of scope and
//! ported by the other per-backend grind items.

use std::ops::Range;

use serde::Serialize;

/// Default chunked-translation prompt template (ports `TRANSLATION_PROMPT`).
///
/// `{source_lang}`, `{target_lang}` and `{text}` are substituted by
/// [`format_prompt`]. The body ends with `Text to translate:\n{text}` so the
/// joined batch (separator-token-delimited cues) lands as the payload.
pub const TRANSLATION_PROMPT: &str = "Translate the following subtitle text from {source_lang} to {target_lang}.\n\nRules:\n- Only output the translated text, nothing else\n- Preserve line breaks where they appear\n- Maintain natural speech patterns suitable for subtitles\n- Keep the same number of subtitle blocks (separated by ---BREAK---)\n\nText to translate:\n{text}";

/// Separator token joining SRT cue contents within a batch (ports the
/// `separator_token="---BREAK---"` default used by `_translate_chunk`).
pub const SRT_SEPARATOR_TOKEN: &str = "---BREAK---";

/// Separator token used for WebVTT/ASS cue batches (ports
/// `separator_token="|||SUBTITLE_BREAK|||"`).
pub const VTT_SEPARATOR_TOKEN: &str = "|||SUBTITLE_BREAK|||";

/// Substitute `{source_lang}`, `{target_lang}` and `{text}` into a prompt
/// template, mirroring Python's `template.format(...)`.
///
/// Only these three placeholders are replaced, in a single left-to-right pass,
/// so literal braces elsewhere in the template are left untouched (the ported
/// templates contain none beyond the three placeholders).
pub fn format_prompt(template: &str, source_lang: &str, target_lang: &str, text: &str) -> String {
    template
        .replace("{source_lang}", source_lang)
        .replace("{target_lang}", target_lang)
        .replace("{text}", text)
}

/// A translation backend (ports `TranslationBackendBase`).
///
/// Implementors provide only [`complete`](Backend::complete) — sending a
/// fully-formed prompt to the model and returning the reply text. The provided
/// [`translate`](Backend::translate) builds the prompt from the shared template,
/// exactly as the Python base class does.
pub trait Backend {
    /// Send a fully-formed prompt to the model and return the reply text.
    ///
    /// Ports `TranslationBackendBase._complete`. Implementations strip the
    /// reply (the Python backends call `.strip()` before returning); the
    /// chunking layer does not re-strip the whole reply.
    fn complete(&self, prompt: &str) -> Result<String, BackendError>;

    /// Translate `text` from `source_lang` to `target_lang`.
    ///
    /// Ports `TranslationBackendBase.translate`: format the prompt (defaulting
    /// to [`TRANSLATION_PROMPT`]) then delegate to [`complete`](Backend::complete).
    fn translate(
        &self,
        text: &str,
        source_lang: &str,
        target_lang: &str,
        prompt_template: Option<&str>,
    ) -> Result<String, BackendError> {
        let template = prompt_template.unwrap_or(TRANSLATION_PROMPT);
        let prompt = format_prompt(template, source_lang, target_lang, text);
        self.complete(&prompt)
    }
}

/// Error returned by a [`Backend`] when completion fails.
///
/// The per-backend grind items extend this with transport-specific variants;
/// here it is the minimal surface the chunking machinery needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackendError {
    /// The backend SDK / optional dependency was not available.
    NotInstalled(String),
    /// The backend call itself failed.
    Request(String),
}

impl std::fmt::Display for BackendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BackendError::NotInstalled(msg) => write!(f, "backend not installed: {msg}"),
            BackendError::Request(msg) => write!(f, "backend request failed: {msg}"),
        }
    }
}

impl std::error::Error for BackendError {}

/// Default Ollama model (ports `OllamaBackend.__init__`'s `model="llama3.2"`).
pub const DEFAULT_OLLAMA_MODEL: &str = "llama3.2";

/// Default Ollama host (ports `base_url="http://localhost:11434"`).
pub const DEFAULT_OLLAMA_URL: &str = "http://localhost:11434";

/// One chat message in the Ollama request body.
///
/// The Python backend sends a single `{"role": "user", "content": prompt}`
/// message; the `ollama` client's pydantic `Message` model serialises only
/// these two fields when the rest are unset (`model_dump(exclude_none=True)`).
#[derive(Serialize)]
struct OllamaMessage<'a> {
    role: &'a str,
    content: &'a str,
}

/// Body of `POST /api/chat`, matching the `ollama` Python client's wire shape.
///
/// The Python `client.chat(model=..., messages=[...])` call builds a
/// `ChatRequest` and serialises it with `model_dump(exclude_none=True)`. With
/// only `model` and `messages` supplied, the resulting body carries exactly
/// `model`, `messages`, `stream` (defaulting to `false`) and `tools` (an empty
/// list, since `tools` defaults to `[]` rather than `None` and so survives the
/// `exclude_none` filter). Every other field is `None` and is dropped.
#[derive(Serialize)]
struct OllamaChatRequest<'a> {
    model: &'a str,
    messages: Vec<OllamaMessage<'a>>,
    stream: bool,
    tools: Vec<serde_json::Value>,
}

/// Subset of the `POST /api/chat` response we read back.
///
/// The Python backend extracts `response["message"]["content"]`; the rest of
/// the `ChatResponse` is ignored.
#[derive(serde::Deserialize)]
struct OllamaChatResponse {
    message: OllamaResponseMessage,
}

#[derive(serde::Deserialize)]
struct OllamaResponseMessage {
    content: String,
}

/// Ollama-based translation backend (ports `OllamaBackend`).
///
/// The default local, free, private backend. [`complete`](Backend::complete)
/// POSTs the prompt as a single user message to `{base_url}/api/chat` over raw
/// [`reqwest`], then returns the stripped `message.content` from the reply,
/// mirroring the Python backend's `client.chat(...)["message"]["content"].strip()`.
pub struct OllamaBackend {
    model: String,
    base_url: String,
    http: reqwest::blocking::Client,
}

impl OllamaBackend {
    /// Construct a backend for `model` against the Ollama server at `base_url`.
    ///
    /// Ports `OllamaBackend(model, base_url)`. Pass [`DEFAULT_OLLAMA_MODEL`] /
    /// [`DEFAULT_OLLAMA_URL`] to reproduce the Python defaults.
    pub fn new(model: impl Into<String>, base_url: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            base_url: base_url.into(),
            http: reqwest::blocking::Client::new(),
        }
    }
}

impl Default for OllamaBackend {
    fn default() -> Self {
        Self::new(DEFAULT_OLLAMA_MODEL, DEFAULT_OLLAMA_URL)
    }
}

impl Backend for OllamaBackend {
    fn complete(&self, prompt: &str) -> Result<String, BackendError> {
        let url = format!("{}/api/chat", self.base_url.trim_end_matches('/'));
        let body = OllamaChatRequest {
            model: &self.model,
            messages: vec![OllamaMessage {
                role: "user",
                content: prompt,
            }],
            stream: false,
            tools: Vec::new(),
        };

        let response = self
            .http
            .post(&url)
            .json(&body)
            .send()
            .map_err(|e| BackendError::Request(e.to_string()))?
            .error_for_status()
            .map_err(|e| BackendError::Request(e.to_string()))?
            .json::<OllamaChatResponse>()
            .map_err(|e| BackendError::Request(e.to_string()))?;

        Ok(response.message.content.trim().to_string())
    }
}

/// The half-open index ranges for each batch when splitting `len` items into
/// chunks of `chunk_size` (ports the `total_chunks` / `start_idx`..`end_idx`
/// loop in `translate_subtitles`).
///
/// Produces `ceil(len / chunk_size)` ranges; the final range is short when
/// `len` is not a multiple of `chunk_size`. A `chunk_size` of zero is treated
/// as a single all-encompassing batch rather than dividing by zero (the Python
/// config validator keeps `chunk_size >= 1`, so this only guards misuse).
pub fn chunk_ranges(len: usize, chunk_size: usize) -> Vec<Range<usize>> {
    if len == 0 {
        return Vec::new();
    }
    if chunk_size == 0 {
        // A single batch spanning all items (not a Vec of one-per-index ranges).
        let whole: Range<usize> = 0..len;
        return vec![whole];
    }
    let total = len.div_ceil(chunk_size);
    (0..total)
        .map(|i| {
            let start = i * chunk_size;
            let end = (start + chunk_size).min(len);
            start..end
        })
        .collect()
}

/// Join one batch's cue contents into the single string sent to the backend
/// (ports `separator.join(texts)` where `separator = f"\n{separator_token}\n"`).
pub fn join_batch<S: AsRef<str>>(texts: &[S], separator_token: &str) -> String {
    let separator = format!("\n{separator_token}\n");
    texts
        .iter()
        .map(|s| s.as_ref())
        .collect::<Vec<_>>()
        .join(&separator)
}

/// Split a translated batch back into per-cue blocks (ports the realignment in
/// `_translate_batch`).
///
/// The reply is split on the bare `separator_token` and each part is stripped.
/// If the resulting block count does not match `input_count`, alignment is
/// unreliable, so the caller's `originals` are returned unchanged — matching the
/// Python fallback that keeps originals for the whole batch rather than shifting
/// translations onto the wrong cues. The returned vec always has length
/// `originals.len()`.
pub fn split_batch(translated: &str, separator_token: &str, originals: &[String]) -> Vec<String> {
    let parts: Vec<String> = translated
        .split(separator_token)
        .map(|p| p.trim().to_string())
        .collect();

    if parts.len() != originals.len() {
        return originals.to_vec();
    }
    parts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunk_ranges_ceildiv_boundaries() {
        // ceil(7 / 3) == 3 batches; final batch short.
        assert_eq!(chunk_ranges(7, 3), vec![0..3, 3..6, 6..7]);
        // exact multiple.
        assert_eq!(chunk_ranges(6, 3), vec![0..3, 3..6]);
        // single batch when chunk_size >= len.
        assert_eq!(chunk_ranges(3, 50), vec![0..3]);
        // empty input.
        assert_eq!(chunk_ranges(0, 50), Vec::<Range<usize>>::new());
    }

    #[test]
    fn join_uses_newline_wrapped_token() {
        let texts = ["a", "b", "c"];
        assert_eq!(join_batch(&texts, "---BREAK---"), "a\n---BREAK---\nb\n---BREAK---\nc");
    }

    #[test]
    fn split_strips_parts_on_match() {
        let originals = vec!["x".to_string(), "y".to_string()];
        let reply = "  hola  ---BREAK---  mundo  ";
        assert_eq!(split_batch(reply, "---BREAK---", &originals), vec!["hola", "mundo"]);
    }

    #[test]
    fn split_falls_back_on_count_mismatch() {
        let originals = vec!["x".to_string(), "y".to_string(), "z".to_string()];
        // model collapsed three cues into two blocks: keep originals.
        let reply = "uno ---BREAK--- dos";
        assert_eq!(split_batch(reply, "---BREAK---", &originals), originals);
    }

    #[test]
    fn format_prompt_substitutes_three_placeholders() {
        let p = format_prompt(TRANSLATION_PROMPT, "en", "es", "Hello.");
        assert!(p.starts_with("Translate the following subtitle text from en to es."));
        assert!(p.ends_with("Text to translate:\nHello."));
    }

    /// Golden body captured from the `ollama` Python client:
    ///
    /// ```text
    /// ChatRequest(model="llama3.2", messages=[{"role": "user", "content": "Hello"}])
    ///     .model_dump(exclude_none=True)
    /// # -> {"model": "llama3.2", "stream": false,
    /// #     "messages": [{"role": "user", "content": "Hello"}], "tools": []}
    /// ```
    ///
    /// Compared as parsed JSON (key order is irrelevant on the wire).
    fn ollama_payload_golden(model: &str, prompt: &str) -> serde_json::Value {
        serde_json::json!({
            "model": model,
            "stream": false,
            "messages": [{"role": "user", "content": prompt}],
            "tools": [],
        })
    }

    #[test]
    fn ollama_request_shape() {
        use std::sync::mpsc;

        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, Request, ResponseTemplate};

        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        runtime.block_on(async {
            let server = MockServer::start().await;

            // Capture the request body the blocking client actually sends.
            let (tx, rx) = mpsc::channel::<serde_json::Value>();
            Mock::given(method("POST"))
                .and(path("/api/chat"))
                .respond_with(move |req: &Request| {
                    tx.send(req.body_json::<serde_json::Value>().unwrap()).unwrap();
                    ResponseTemplate::new(200).set_body_json(serde_json::json!({
                        "message": {"role": "assistant", "content": "  hola  "},
                    }))
                })
                .mount(&server)
                .await;

            let base_url = server.uri();
            // The blocking reqwest call cannot run on the async runtime thread;
            // drive it on a dedicated thread and await its result.
            let reply = tokio::task::spawn_blocking(move || {
                let backend = OllamaBackend::new(DEFAULT_OLLAMA_MODEL, base_url);
                backend.complete("Hello")
            })
            .await
            .unwrap()
            .unwrap();

            // message.content is returned stripped, matching `.strip()`.
            assert_eq!(reply, "hola");

            let sent = rx.recv().unwrap();
            assert_eq!(sent, ollama_payload_golden(DEFAULT_OLLAMA_MODEL, "Hello"));
        });
    }
}
