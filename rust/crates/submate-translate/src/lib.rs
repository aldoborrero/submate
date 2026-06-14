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
//! Three of the four providers speak the OpenAI chat-completions wire format, so
//! they share a single [`OpenAiCompatBackend`] built on the `async-openai`
//! crate and distinguished only by base URL: OpenAI (the default base), Ollama
//! (`{ollama_url}/v1`) and Gemini (the Generative Language OpenAI-compat
//! endpoint). Anthropic has no trustworthy Rust SDK, so [`AnthropicBackend`] stays
//! a hand-rolled async-[`reqwest`] backend against the messages API.

use std::future::Future;
use std::ops::Range;
use std::time::Duration;

use async_openai::config::{OpenAIConfig, OPENAI_API_BASE};
use async_openai::error::OpenAIError;
use async_openai::types::chat::{
    ChatCompletionRequestUserMessageArgs, CreateChatCompletionRequestArgs,
};
use async_openai::Client;
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
///
/// `complete`/`translate` are `async` via [`async_trait`](async_trait::async_trait),
/// which boxes the returned futures so the trait stays object-safe for
/// `Box<dyn Backend>` — the four async HTTP backends `.await` their
/// `reqwest::Client` directly inside the node's async runtime.
#[async_trait::async_trait]
pub trait Backend {
    /// Stable identifier for the backend (`"ollama"`/`"claude"`/`"openai"`/
    /// `"gemini"`), matching the [`submate_types::TranslationBackend`] string
    /// form. Useful for logging which backend ran.
    fn id(&self) -> &'static str;

    /// Send a fully-formed prompt to the model and return the reply text.
    ///
    /// Ports `TranslationBackendBase._complete`. Implementations strip the
    /// reply (the Python backends call `.strip()` before returning); the
    /// chunking layer does not re-strip the whole reply.
    async fn complete(&self, prompt: &str) -> Result<String, BackendError>;

    /// Translate `text` from `source_lang` to `target_lang`.
    ///
    /// Ports `TranslationBackendBase.translate`: format the prompt (defaulting
    /// to [`TRANSLATION_PROMPT`]) then delegate to [`complete`](Backend::complete).
    async fn translate(
        &self,
        text: &str,
        source_lang: &str,
        target_lang: &str,
        prompt_template: Option<&str>,
    ) -> Result<String, BackendError> {
        let template = prompt_template.unwrap_or(TRANSLATION_PROMPT);
        let prompt = format_prompt(template, source_lang, target_lang, text);
        self.complete(&prompt).await
    }
}

/// Borrowed settings needed to construct a [`Backend`] via [`make_backend`].
///
/// Mirrors the fields the four backend constructors need, so the factory can
/// live here without depending on `submate-config`. Callers (the CLI, the
/// node) borrow each field from their own config struct.
pub struct BackendSettings<'a> {
    /// Which backend to construct.
    pub backend: submate_types::TranslationBackend,
    /// Ollama model name.
    pub ollama_model: &'a str,
    /// Ollama server base URL.
    pub ollama_url: &'a str,
    /// Anthropic API key (for Claude).
    pub anthropic_api_key: &'a str,
    /// Claude model name.
    pub claude_model: &'a str,
    /// OpenAI API key.
    pub openai_api_key: &'a str,
    /// OpenAI model name.
    pub openai_model: &'a str,
    /// Gemini API key.
    pub gemini_api_key: &'a str,
    /// Gemini model name.
    pub gemini_model: &'a str,
}

/// Construct the configured [`Backend`] from [`BackendSettings`].
///
/// Single source of truth for the `TranslationBackend` → `Box<dyn Backend>`
/// mapping, shared by the CLI and the node so neither duplicates the match.
///
/// The boxed backend is `Send + Sync` so the node's pull-loop can hold it as a
/// field across `.await` points. The backends are stateless (just config /
/// client) and issue an async HTTP request per `complete` call, awaited
/// directly on the runtime.
///
/// `Ollama`/`Openai`/`Gemini` all map to an [`OpenAiCompatBackend`] differing
/// only in base URL: Ollama serves the OpenAI-compat API under `{ollama_url}/v1`
/// (no key needed, so a placeholder is sent); OpenAI uses the crate's default
/// base; Gemini uses the Generative Language OpenAI-compat base ending in
/// `/openai`, so `async-openai` appends `/chat/completions`. `Claude` keeps the
/// native [`AnthropicBackend`].
pub fn make_backend(s: &BackendSettings<'_>) -> Box<dyn Backend + Send + Sync> {
    use submate_types::TranslationBackend;

    match s.backend {
        TranslationBackend::Ollama => Box::new(OpenAiCompatBackend::new(
            "ollama",
            // Ollama's OpenAI-compat surface ignores the key, but the client
            // still sends an `Authorization` header; a placeholder keeps it
            // well-formed.
            OLLAMA_PLACEHOLDER_KEY,
            s.ollama_model,
            format!("{}/v1", s.ollama_url.trim_end_matches('/')),
        )),
        TranslationBackend::Openai => Box::new(OpenAiCompatBackend::new(
            "openai",
            s.openai_api_key,
            s.openai_model,
            OPENAI_API_BASE,
        )),
        TranslationBackend::Gemini => Box::new(OpenAiCompatBackend::new(
            "gemini",
            s.gemini_api_key,
            s.gemini_model,
            GEMINI_OPENAI_BASE,
        )),
        TranslationBackend::Claude => {
            Box::new(AnthropicBackend::new(s.anthropic_api_key, s.claude_model))
        }
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
            Self::NotInstalled(msg) => write!(f, "backend not installed: {msg}"),
            Self::Request(msg) => write!(f, "backend request failed: {msg}"),
        }
    }
}

impl std::error::Error for BackendError {}

/// A transport failure (`send`/`error_for_status`/`json`) becomes a
/// [`BackendError::Request`] carrying the [`reqwest::Error`]'s display string,
/// so the four HTTP backends can lean on `?` instead of repeating a `.map_err`.
impl From<reqwest::Error> for BackendError {
    fn from(err: reqwest::Error) -> Self {
        Self::Request(err.to_string())
    }
}

/// An `async-openai` call/build failure becomes a [`BackendError::Request`]
/// carrying the [`OpenAIError`]'s display string, so [`OpenAiCompatBackend`]
/// can lean on `?`.
impl From<OpenAIError> for BackendError {
    fn from(err: OpenAIError) -> Self {
        Self::Request(err.to_string())
    }
}

/// Default Ollama model (ports `OllamaBackend.__init__`'s `model="llama3.2"`).
pub const DEFAULT_OLLAMA_MODEL: &str = "llama3.2";

/// Default Ollama host (ports `base_url="http://localhost:11434"`).
///
/// The [`make_backend`] routing appends `/v1` to reach Ollama's
/// OpenAI-compatible chat endpoint.
pub const DEFAULT_OLLAMA_URL: &str = "http://localhost:11434";

/// Default OpenAI model (ports `OpenAIBackend.__init__`'s `model="gpt-5-mini"`).
pub const DEFAULT_OPENAI_MODEL: &str = "gpt-5-mini";

/// Default Gemini model (ports `GeminiBackend.__init__`'s
/// `model="gemini-2.5-flash"`).
pub const DEFAULT_GEMINI_MODEL: &str = "gemini-2.5-flash";

/// Base URL for Gemini's OpenAI-compatible endpoint.
///
/// The Generative Language API exposes a chat-completions surface under
/// `.../v1beta/openai`; because it ends in `/openai` (no trailing slash),
/// `async-openai`'s `url(path)` appends `/chat/completions` to form the final
/// request URL.
pub const GEMINI_OPENAI_BASE: &str = "https://generativelanguage.googleapis.com/v1beta/openai";

/// Placeholder API key sent to Ollama, whose OpenAI-compat surface ignores the
/// `Authorization` header but still expects it to be present.
const OLLAMA_PLACEHOLDER_KEY: &str = "ollama";

/// Connect timeout for the LLM backends: catch an unreachable endpoint quickly
/// instead of waiting out the whole request budget on a dead host.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Overall per-request timeout for the LLM backends. Generous enough not to cut
/// off a legitimately slow generation (e.g. Ollama on CPU), but bounded so a
/// hung or non-responding endpoint can't stall a worker forever — the previous
/// code set no timeout at all, so a single stuck call blocked the whole batch.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(300);

/// Build the shared `reqwest::Client` for the LLM backends with connect/request
/// timeouts. Constructed once per backend and reused across every chunk request
/// (so connection pooling and TLS sessions are kept). A failure here means the
/// system TLS stack is unusable — fatal, and surfaced once at construction
/// rather than silently per request.
fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(REQUEST_TIMEOUT)
        .build()
        .expect("reqwest client (system TLS backend) must initialize")
}

/// OpenAI-compatible translation backend, shared by OpenAI, Ollama and Gemini.
///
/// Wraps an `async-openai` [`Client`] configured by `base_url` + API key, so the
/// three providers differ only in those two values (and the model name). Ports
/// the Python `OpenAIBackend`/`OllamaBackend`/`GeminiBackend` `_complete`:
/// [`complete`](Backend::complete) sends the prompt as a single user message via
/// `POST {base_url}/chat/completions` and returns the stripped
/// `choices[0].message.content` (empty string when null).
pub struct OpenAiCompatBackend {
    id: &'static str,
    client: Client<OpenAIConfig>,
    model: String,
    base_url: String,
}

impl OpenAiCompatBackend {
    /// Construct a backend identified by `id` (`"openai"`/`"ollama"`/`"gemini"`)
    /// for `model`, authenticating with `api_key` against `base_url`.
    pub fn new(
        id: &'static str,
        api_key: impl Into<String>,
        model: impl Into<String>,
        base_url: impl Into<String>,
    ) -> Self {
        let base_url = base_url.into();
        let config = OpenAIConfig::new()
            .with_api_key(api_key.into())
            .with_api_base(base_url.clone());
        Self {
            id,
            // Drive async-openai through our timeout-configured client so a hung
            // provider can't block forever (its default client has no timeout).
            client: Client::with_config(config).with_http_client(http_client()),
            model: model.into(),
            base_url,
        }
    }

    /// The configured API base URL (the value `async-openai` prefixes onto
    /// `/chat/completions`). Used by the factory-routing test to assert each
    /// variant lands on the right provider.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }
}

#[async_trait::async_trait]
impl Backend for OpenAiCompatBackend {
    fn id(&self) -> &'static str {
        self.id
    }

    async fn complete(&self, prompt: &str) -> Result<String, BackendError> {
        let request = CreateChatCompletionRequestArgs::default()
            .model(&self.model)
            .messages([ChatCompletionRequestUserMessageArgs::default()
                .content(prompt)
                .build()?
                .into()])
            .build()?;

        let response = self.client.chat().create(request).await?;
        let content = response
            .choices
            .into_iter()
            .next()
            .and_then(|c| c.message.content)
            .unwrap_or_default();
        Ok(content.trim().to_string())
    }
}

/// Default Claude model (ports `ClaudeBackend.__init__`'s
/// `model="claude-sonnet-4-6"`).
pub const DEFAULT_CLAUDE_MODEL: &str = "claude-sonnet-4-6";

/// Default Anthropic messages API base URL.
///
/// The Python backend uses the `anthropic` SDK, whose default base is
/// `https://api.anthropic.com`; the messages endpoint is `{base}/v1/messages`.
pub const DEFAULT_ANTHROPIC_URL: &str = "https://api.anthropic.com";

/// `anthropic-version` header value sent by the `anthropic` SDK.
const ANTHROPIC_VERSION: &str = "2023-06-01";

/// `max_tokens` value the Python `ClaudeBackend` hard-codes on every request.
const CLAUDE_MAX_TOKENS: u32 = 4096;

/// One chat message in a Claude / OpenAI request body (`{role, content}`).
#[derive(Serialize)]
struct ChatMessage<'a> {
    role: &'a str,
    content: &'a str,
}

/// Body of `POST /v1/messages`, matching the Anthropic messages API.
///
/// Mirrors the Python `client.messages.create(model=..., max_tokens=4096,
/// messages=[{"role": "user", "content": prompt}])` call.
#[derive(Serialize)]
struct ClaudeRequest<'a> {
    model: &'a str,
    max_tokens: u32,
    messages: Vec<ChatMessage<'a>>,
}

/// Subset of the messages API response we read back.
///
/// The Python backend walks `message.content` and returns the first block that
/// has a `text` attribute (the `TextBlock`), ignoring other block types
/// (e.g. tool-use blocks). We deserialise each block's optional `text` and pick
/// the first one present.
#[derive(serde::Deserialize)]
struct ClaudeResponse {
    content: Vec<ClaudeBlock>,
}

#[derive(serde::Deserialize)]
struct ClaudeBlock {
    #[serde(default)]
    text: Option<String>,
}

/// Claude/Anthropic translation backend (ports `ClaudeBackend`).
///
/// [`complete`](Backend::complete) POSTs the prompt as a single user message to
/// `{base_url}/v1/messages` with the `x-api-key` and `anthropic-version`
/// headers, then returns the stripped text of the first content block that
/// carries text — mirroring the Python loop over `message.content` that returns
/// the first block with a `text` attribute.
pub struct AnthropicBackend {
    api_key: String,
    model: String,
    base_url: String,
    client: reqwest::Client,
}

impl AnthropicBackend {
    /// Construct a Claude backend for `model` authenticating with `api_key`
    /// against the default Anthropic base URL.
    ///
    /// Ports `ClaudeBackend(api_key, model)`. Pass [`DEFAULT_CLAUDE_MODEL`] to
    /// reproduce the Python default.
    pub fn new(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self::with_base_url(api_key, model, DEFAULT_ANTHROPIC_URL)
    }

    /// Construct a Claude backend pointed at an explicit `base_url` (for tests).
    pub fn with_base_url(
        api_key: impl Into<String>,
        model: impl Into<String>,
        base_url: impl Into<String>,
    ) -> Self {
        Self {
            api_key: api_key.into(),
            model: model.into(),
            base_url: base_url.into(),
            client: http_client(),
        }
    }
}

#[async_trait::async_trait]
impl Backend for AnthropicBackend {
    fn id(&self) -> &'static str {
        "claude"
    }

    async fn complete(&self, prompt: &str) -> Result<String, BackendError> {
        let url = format!("{}/v1/messages", self.base_url.trim_end_matches('/'));
        let body = ClaudeRequest {
            model: &self.model,
            max_tokens: CLAUDE_MAX_TOKENS,
            messages: vec![ChatMessage {
                role: "user",
                content: prompt,
            }],
        };

        let response = self
            .client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .json(&body)
            .send()
            .await?
            .error_for_status()?
            .json::<ClaudeResponse>()
            .await?;

        let text = response
            .content
            .into_iter()
            .find_map(|block| block.text)
            .unwrap_or_default();
        Ok(text.trim().to_string())
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

/// ASS/SSA tag-preservation prompt (ports `ASS_TRANSLATION_PROMPT`).
///
/// Used by [`translate_ass_dialogue`] instead of [`TRANSLATION_PROMPT`]: it
/// instructs the model to translate only the human-readable dialogue while
/// leaving `{...}` override tags and `\N` / `\n` newline markers untouched.
/// The literal `{{...}}` braces in the Python f-string-style template are
/// single braces here (Rust does not escape them).
pub const ASS_TRANSLATION_PROMPT: &str = "Translate the following ASS subtitle dialogue from {source_lang} to {target_lang}.\n\nCRITICAL RULES:\n1. ONLY translate the human-readable dialogue text\n2. PRESERVE ALL formatting tags exactly as-is: {\\i1}, {\\b1}, {\\pos(x,y)}, {\\an8}, {\\fad(x,y)}, etc.\n3. PRESERVE newline markers: \\N and \\n\n4. PRESERVE the exact line structure (one subtitle per line, separated by |||SUBTITLE_BREAK|||)\n5. DO NOT add, remove, or modify any tags inside curly braces {}\n6. DO NOT translate or modify anything inside curly braces {}\n7. Output ONLY the translated subtitles, no explanations\n\nExample input:\n{\\i1}Bonjour{\\i0} monde\n|||SUBTITLE_BREAK|||\n{\\an8}Comment ça va?\n\nExample output:\n{\\i1}Hello{\\i0} world\n|||SUBTITLE_BREAK|||\n{\\an8}How are you?\n\nSubtitles to translate:\n{text}";

/// Extract every `{...}` override-tag substring from an ASS dialogue line, in
/// order. Ports the `re.findall(r"\{[^}]*\}", text)` in `validate_ass_tags`:
/// each match starts at a `{` and runs to the next `}` (empty bodies allowed).
fn ass_tags(text: &str) -> Vec<&str> {
    let bytes = text.as_bytes();
    let mut tags = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'{' {
            if let Some(rel) = text[i + 1..].find('}') {
                let end = i + 1 + rel; // index of the closing '}'
                tags.push(&text[i..=end]);
                i = end + 1;
                continue;
            }
            // No closing brace: the regex cannot match here; stop scanning.
            break;
        }
        i += 1;
    }
    tags
}

/// Whether `translated` preserves the exact ASS override tags of `original`
/// (same `{...}` substrings, in the same order). Ports `validate_ass_tags`.
pub fn validate_ass_tags(original: &str, translated: &str) -> bool {
    ass_tags(original) == ass_tags(translated)
}

/// Translate a batch of cue texts in one model round-trip and realign the
/// result (ports `TranslationService._translate_batch`).
///
/// Joins `texts` with the newline-wrapped `separator_token` ([`join_batch`]),
/// awaits `complete` on the formatted prompt, then splits the reply back into
/// per-cue blocks ([`split_batch`]). On a block-count mismatch the originals are
/// kept for the whole batch. `complete` is an async callback that receives the
/// fully-formed prompt and returns the model reply (already stripped, as the
/// backends do); decoupling it from [`Backend`] lets callers drive the flow from
/// a recorded map in tests.
async fn translate_batch<E, F, Fut>(
    texts: &[String],
    source_lang: &str,
    target_lang: &str,
    separator_token: &str,
    prompt_template: &str,
    complete: &mut F,
) -> Result<Vec<String>, E>
where
    F: FnMut(String) -> Fut,
    Fut: Future<Output = Result<String, E>>,
{
    let combined = join_batch(texts, separator_token);
    let prompt = format_prompt(prompt_template, source_lang, target_lang, &combined);
    let translated = complete(prompt).await?;
    Ok(split_batch(&translated, separator_token, texts))
}

/// Run the chunked batch-translation loop over `texts`, returning a
/// translation aligned 1:1 with the input (ports the `chunk_size` loop shared by
/// `translate_subtitles`, `translate_vtt_content` and `translate_ass_content`).
///
/// Splits `texts` into batches of `chunk_size` ([`chunk_ranges`]), translating
/// each via [`translate_batch`]. The returned vec has the same length as
/// `texts`; a `chunk_size` of zero translates everything in a single batch.
async fn translate_chunks<E, F, Fut>(
    texts: &[String],
    source_lang: &str,
    target_lang: &str,
    chunk_size: usize,
    separator_token: &str,
    prompt_template: &str,
    complete: &mut F,
) -> Result<Vec<String>, E>
where
    F: FnMut(String) -> Fut,
    Fut: Future<Output = Result<String, E>>,
{
    let mut out = Vec::with_capacity(texts.len());
    for range in chunk_ranges(texts.len(), chunk_size) {
        let batch = &texts[range];
        let translated = translate_batch(
            batch,
            source_lang,
            target_lang,
            separator_token,
            prompt_template,
            complete,
        )
        .await?;
        out.extend(translated);
    }
    Ok(out)
}

/// Translate raw SRT content, preserving cue indices and timing (ports
/// `TranslationService.translate_srt_content`).
///
/// Short-circuits and returns the input unchanged when `source_lang ==
/// target_lang`. Otherwise parses with [`submate_subtitle::cue::parse_srt`],
/// translates the cue contents in chunks of `chunk_size` (joined with
/// [`SRT_SEPARATOR_TOKEN`] under [`TRANSLATION_PROMPT`]), writes the results
/// back onto the cues, and re-emits via [`submate_subtitle::cue::compose_srt`].
/// `complete` is awaited once per batch with the fully-formed prompt.
pub async fn translate_srt_content<E, F, Fut>(
    srt_content: &str,
    source_lang: &str,
    target_lang: &str,
    chunk_size: usize,
    complete: &mut F,
) -> Result<String, E>
where
    F: FnMut(String) -> Fut,
    Fut: Future<Output = Result<String, E>>,
{
    if source_lang == target_lang {
        return Ok(srt_content.to_string());
    }

    let mut cues = submate_subtitle::cue::parse_srt(srt_content);
    let texts: Vec<String> = cues.iter().map(|c| c.text.clone()).collect();
    let translated = translate_chunks(
        &texts,
        source_lang,
        target_lang,
        chunk_size,
        SRT_SEPARATOR_TOKEN,
        TRANSLATION_PROMPT,
        complete,
    )
    .await?;
    for (cue, text) in cues.iter_mut().zip(translated) {
        cue.text = text;
    }
    Ok(submate_subtitle::cue::compose_srt(&cues))
}

/// Translate raw WebVTT content, preserving cue timing and structure (ports
/// `TranslationService.translate_vtt_content`).
///
/// Mirrors [`translate_srt_content`] but parses/serializes with
/// [`submate_subtitle::cue::parse_vtt`] / [`compose_vtt`] and joins cues with
/// [`VTT_SEPARATOR_TOKEN`]. Like the Python port, when the parse yields no
/// translatable cues the input is returned unchanged.
///
/// [`compose_vtt`]: submate_subtitle::cue::compose_vtt
pub async fn translate_vtt_content<E, F, Fut>(
    vtt_content: &str,
    source_lang: &str,
    target_lang: &str,
    chunk_size: usize,
    complete: &mut F,
) -> Result<String, E>
where
    F: FnMut(String) -> Fut,
    Fut: Future<Output = Result<String, E>>,
{
    if source_lang == target_lang {
        return Ok(vtt_content.to_string());
    }

    let mut cues = submate_subtitle::cue::parse_vtt(vtt_content);
    if cues.is_empty() {
        return Ok(vtt_content.to_string());
    }

    let texts: Vec<String> = cues.iter().map(|c| c.text.clone()).collect();
    let translated = translate_chunks(
        &texts,
        source_lang,
        target_lang,
        chunk_size,
        VTT_SEPARATOR_TOKEN,
        TRANSLATION_PROMPT,
        complete,
    )
    .await?;
    for (cue, text) in cues.iter_mut().zip(translated) {
        cue.text = text;
    }
    Ok(submate_subtitle::cue::compose_vtt(&cues))
}

/// Translate already-extracted ASS dialogue lines, dropping any translation that
/// would alter the line's override tags (ports the tag-preservation body of
/// `TranslationService.translate_ass_content`).
///
/// The workspace has no ASS (de)serializer, so this ports the portable core:
/// given the dialogue `texts` pysubs2 would have extracted, it translates them
/// in chunks (joined with [`VTT_SEPARATOR_TOKEN`] under
/// [`ASS_TRANSLATION_PROMPT`]) and, per line, keeps the translation only when
/// [`validate_ass_tags`] confirms the `{...}` tags are unchanged — otherwise it
/// keeps the original, matching the Python "tag mismatch, keeping original"
/// fallback. The returned vec aligns 1:1 with `texts`.
pub async fn translate_ass_dialogue<E, F, Fut>(
    texts: &[String],
    source_lang: &str,
    target_lang: &str,
    chunk_size: usize,
    complete: &mut F,
) -> Result<Vec<String>, E>
where
    F: FnMut(String) -> Fut,
    Fut: Future<Output = Result<String, E>>,
{
    if source_lang == target_lang {
        return Ok(texts.to_vec());
    }

    let translated = translate_chunks(
        texts,
        source_lang,
        target_lang,
        chunk_size,
        VTT_SEPARATOR_TOKEN,
        ASS_TRANSLATION_PROMPT,
        complete,
    )
    .await?;

    Ok(texts
        .iter()
        .zip(translated)
        .map(|(original, new_text)| {
            if validate_ass_tags(original, &new_text) {
                new_text
            } else {
                original.clone()
            }
        })
        .collect())
}

/// Translate a plain-text blob in a single round-trip (ports
/// `TranslationService.translate_text`).
///
/// Short-circuits and returns `text` unchanged when `source_lang ==
/// target_lang` (the same no-op guard the Python method applies before touching
/// the backend). Otherwise it issues exactly one `complete` call with the
/// default [`TRANSLATION_PROMPT`] (no separator-token batching — the plain-text
/// path mirrors `backend.translate(..., prompt_template=None)`) and returns the
/// reply.
pub async fn translate_text<E, F, Fut>(
    text: &str,
    source_lang: &str,
    target_lang: &str,
    complete: &mut F,
) -> Result<String, E>
where
    F: FnMut(String) -> Fut,
    Fut: Future<Output = Result<String, E>>,
{
    if source_lang == target_lang {
        return Ok(text.to_string());
    }
    let prompt = format_prompt(TRANSLATION_PROMPT, source_lang, target_lang, text);
    complete(prompt).await
}

/// Per-format translation dispatch for already-formatted Bazarr output (ports
/// `BazarrService._translate_content`).
///
/// Decides *how* to translate already-rendered subtitle `content` for the
/// requested [`OutputFormat`], and *when* to skip translation entirely. The
/// contract Bazarr relies on is that this never propagates a failure: it always
/// returns well-formed content for the requested format, falling back to the
/// untranslated input whenever translation is unavailable, unsupported, or
/// errors.
///
/// 1. **Short-circuit guard** — empty `content` or `source_lang ==
///    target_lang` returns `content` unchanged, *before* any LLM call (the
///    closure is not invoked).
/// 2. **Format dispatch** —
///    - [`OutputFormat::Srt`] → [`translate_srt_content`]
///    - [`OutputFormat::Vtt`] → [`translate_vtt_content`]
///    - [`OutputFormat::Txt`] → [`translate_text`] (plain [`TRANSLATION_PROMPT`],
///      no batching)
///    - [`OutputFormat::Json`] → **skip**: the JSON dump holds the full result,
///      so it is returned unchanged without calling the closure (Python logs
///      "Translation not supported for JSON format").
/// 3. **Exception fallback** — any `Err` raised by the dispatched translation
///    is swallowed and the original `content` is returned, matching the Python
///    `try/except` that degrades to the untranslated text rather than failing
///    the Bazarr request.
///
/// `chunk_size` is forwarded to the SRT/VTT batch loop; `complete` is the
/// closure-driven LLM entrypoint shared with the sibling translate fns. The
/// return type is `String` (not `Result`) precisely because the fallback
/// absorbs every error into the verbatim `content`.
pub async fn translate_content<E, F, Fut>(
    content: &str,
    source_lang: &str,
    target_lang: &str,
    output_format: submate_queue::models::OutputFormat,
    chunk_size: usize,
    complete: &mut F,
) -> String
where
    F: FnMut(String) -> Fut,
    Fut: Future<Output = Result<String, E>>,
{
    use submate_queue::models::OutputFormat;

    if content.is_empty() || source_lang == target_lang {
        return content.to_string();
    }

    let result: Result<String, E> = match output_format {
        OutputFormat::Srt => {
            translate_srt_content(content, source_lang, target_lang, chunk_size, complete).await
        }
        OutputFormat::Vtt => {
            translate_vtt_content(content, source_lang, target_lang, chunk_size, complete).await
        }
        OutputFormat::Txt => translate_text(content, source_lang, target_lang, complete).await,
        // JSON holds the full result dump; translation is unsupported, so the
        // closure is never called and the input is returned verbatim.
        OutputFormat::Json => return content.to_string(),
    };

    // Translation failure must never reach the Bazarr caller: fall back to the
    // original, untranslated content.
    result.unwrap_or_else(|_| content.to_string())
}

#[cfg(test)]
mod tests {
    use std::convert::Infallible;

    use super::*;

    #[test]
    fn backend_factory_ids() {
        use submate_types::TranslationBackend;

        let cases = [
            (TranslationBackend::Ollama, "ollama"),
            (TranslationBackend::Claude, "claude"),
            (TranslationBackend::Openai, "openai"),
            (TranslationBackend::Gemini, "gemini"),
        ];
        for (backend, expected) in cases {
            let settings = BackendSettings {
                backend,
                ollama_model: "m",
                ollama_url: "http://localhost:11434",
                anthropic_api_key: "k",
                claude_model: "m",
                openai_api_key: "k",
                openai_model: "m",
                gemini_api_key: "k",
                gemini_model: "m",
            };
            assert_eq!(make_backend(&settings).id(), expected);
        }
    }

    /// Each `OpenAiCompatBackend` variant is wired to the base URL from the
    /// routing table: OpenAI → the crate default, Ollama → `{url}/v1`, Gemini →
    /// the Generative Language OpenAI-compat base. Built the same way as
    /// [`make_backend`] (the factory returns `Box<dyn Backend>`, which hides the
    /// `base_url()` accessor).
    #[test]
    fn backend_factory_routing() {
        let openai = OpenAiCompatBackend::new("openai", "k", DEFAULT_OPENAI_MODEL, OPENAI_API_BASE);
        assert_eq!(openai.id(), "openai");
        assert_eq!(openai.base_url(), "https://api.openai.com/v1");

        let ollama = OpenAiCompatBackend::new(
            "ollama",
            OLLAMA_PLACEHOLDER_KEY,
            DEFAULT_OLLAMA_MODEL,
            format!("{}/v1", DEFAULT_OLLAMA_URL.trim_end_matches('/')),
        );
        assert_eq!(ollama.id(), "ollama");
        assert_eq!(ollama.base_url(), "http://localhost:11434/v1");

        let gemini = OpenAiCompatBackend::new("gemini", "k", DEFAULT_GEMINI_MODEL, GEMINI_OPENAI_BASE);
        assert_eq!(gemini.id(), "gemini");
        assert_eq!(
            gemini.base_url(),
            "https://generativelanguage.googleapis.com/v1beta/openai"
        );
    }

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
        assert_eq!(
            join_batch(&texts, "---BREAK---"),
            "a\n---BREAK---\nb\n---BREAK---\nc"
        );
    }

    #[test]
    fn split_strips_parts_on_match() {
        let originals = vec!["x".to_string(), "y".to_string()];
        let reply = "  hola  ---BREAK---  mundo  ";
        assert_eq!(
            split_batch(reply, "---BREAK---", &originals),
            vec!["hola", "mundo"]
        );
    }

    #[test]
    fn split_falls_back_on_count_mismatch() {
        let originals = vec!["x".to_string(), "y".to_string(), "z".to_string()];
        // model collapsed three cues into two blocks: keep originals.
        let reply = "uno ---BREAK--- dos";
        assert_eq!(split_batch(reply, "---BREAK---", &originals), originals);
    }

    #[test]
    fn ass_tags_match_when_only_dialogue_changes() {
        // Same `{...}` tags in the same order -> preserved.
        assert!(validate_ass_tags(
            "{\\i1}Bonjour{\\i0} monde",
            "{\\i1}Hello{\\i0} world"
        ));
        // A dropped tag -> rejected.
        assert!(!validate_ass_tags("{\\i1}Bonjour{\\i0}", "{\\i1}Hello"));
        // A reordered/changed tag -> rejected.
        assert!(!validate_ass_tags("{\\an8}Hi", "{\\an2}Hi"));
        // No tags either side -> trivially preserved.
        assert!(validate_ass_tags("plain", "llano"));
    }

    #[tokio::test]
    async fn translate_srt_short_circuits_on_same_language() {
        let srt = "1\n00:00:01,000 --> 00:00:02,000\nHi\n\n";
        let mut complete = async |_: String| -> Result<String, Infallible> {
            panic!("backend must not be called when source == target");
        };
        let out = translate_srt_content(srt, "en", "en", 50, &mut complete)
            .await
            .unwrap();
        assert_eq!(out, srt);
    }

    #[tokio::test]
    async fn ass_dialogue_keeps_original_on_tag_mismatch() {
        // Two cues in one batch; the model drops the tag on the second.
        let texts = vec!["{\\i1}Hello".to_string(), "{\\b1}World".to_string()];
        let mut complete = async |_: String| -> Result<String, Infallible> {
            Ok(format!("{{\\i1}}Hola{VTT_SEPARATOR_TOKEN}Mundo"))
        };
        let out = translate_ass_dialogue(&texts, "en", "es", 50, &mut complete)
            .await
            .unwrap();
        // First cue's tags preserved -> translation kept; second mismatched ->
        // original kept.
        assert_eq!(
            out,
            vec!["{\\i1}Hola".to_string(), "{\\b1}World".to_string()]
        );
    }

    #[tokio::test]
    async fn translate_text_short_circuits_on_same_language() {
        let mut complete = async |_: String| -> Result<String, Infallible> {
            panic!("backend must not be called when source == target");
        };
        let out = translate_text("hola", "es", "es", &mut complete)
            .await
            .unwrap();
        assert_eq!(out, "hola");
    }

    #[tokio::test]
    async fn translate_text_single_request_uses_default_prompt() {
        let seen_prompt: std::cell::RefCell<Option<String>> = std::cell::RefCell::new(None);
        let mut complete = async |prompt: String| -> Result<String, Infallible> {
            *seen_prompt.borrow_mut() = Some(prompt);
            Ok("Hello.".to_string())
        };
        let out = translate_text("Hola.", "es", "en", &mut complete)
            .await
            .unwrap();
        assert_eq!(out, "Hello.");
        let prompt = seen_prompt
            .into_inner()
            .expect("backend was called exactly once");
        // Plain-text path: default TRANSLATION_PROMPT, payload is the bare text
        // with no separator-token batching (the input lands verbatim after the
        // "Text to translate:" header).
        assert!(prompt.starts_with("Translate the following subtitle text from es to en."));
        assert!(prompt.ends_with("Text to translate:\nHola."));
    }

    /// Falsifiers for `translate_content`'s per-format dispatch, the JSON skip,
    /// the same-lang/empty short-circuits, and the exception fallback. Each case
    /// records whether the LLM closure was invoked so the no-op branches are
    /// proven to bypass it, and the fallback case returns an `Err` to assert the
    /// verbatim-content recovery.
    mod dispatch {
        use std::convert::Infallible;

        use submate_queue::models::OutputFormat;

        use super::super::{translate_content, SRT_SEPARATOR_TOKEN, VTT_SEPARATOR_TOKEN};

        const SRT_IN: &str = "1\n00:00:01,000 --> 00:00:02,000\nHello\n\n";
        const VTT_IN: &str = "WEBVTT\n\n00:00:01.000 --> 00:00:02.000\nHello\n";

        /// SRT dispatch: the SRT batch path runs (the closure sees the
        /// `---BREAK---` separator-joined payload) and the cue text is replaced.
        #[tokio::test]
        async fn srt_branch_runs_srt_translator() {
            let called = std::cell::Cell::new(false);
            let mut complete = async |prompt: String| -> Result<String, Infallible> {
                called.set(true);
                assert!(prompt.contains(SRT_SEPARATOR_TOKEN) || prompt.contains("Hello"));
                Ok("Hola".to_string())
            };
            let out =
                translate_content(SRT_IN, "en", "es", OutputFormat::Srt, 50, &mut complete).await;
            assert!(called.get(), "SRT branch must call the LLM");
            assert!(out.contains("Hola"));
            assert!(out.contains("00:00:01,000 --> 00:00:02,000"));
        }

        /// VTT dispatch: the VTT batch path runs (the `|||SUBTITLE_BREAK|||`
        /// separator) and the cue text is replaced.
        #[tokio::test]
        async fn vtt_branch_runs_vtt_translator() {
            let called = std::cell::Cell::new(false);
            let mut complete = async |prompt: String| -> Result<String, Infallible> {
                called.set(true);
                assert!(prompt.contains(VTT_SEPARATOR_TOKEN) || prompt.contains("Hello"));
                Ok("Hola".to_string())
            };
            let out =
                translate_content(VTT_IN, "en", "es", OutputFormat::Vtt, 50, &mut complete).await;
            assert!(called.get(), "VTT branch must call the LLM");
            assert!(out.contains("Hola"));
        }

        /// TXT dispatch: the plain-text path runs (single request, no separator
        /// token) and returns the reply verbatim.
        #[tokio::test]
        async fn txt_branch_runs_plain_translator() {
            let call_count = std::cell::Cell::new(0u32);
            let mut complete = async |prompt: String| -> Result<String, Infallible> {
                call_count.set(call_count.get() + 1);
                // Plain-text path: the bare blob lands as the payload, not
                // separator-joined cues.
                assert!(prompt.ends_with("Text to translate:\nHello world"));
                Ok("Hola mundo".to_string())
            };
            let out = translate_content(
                "Hello world",
                "en",
                "es",
                OutputFormat::Txt,
                50,
                &mut complete,
            )
            .await;
            assert_eq!(call_count.get(), 1, "TXT path issues exactly one request");
            assert_eq!(out, "Hola mundo");
        }

        /// JSON skip: the closure is never invoked and the JSON content is
        /// returned byte-for-byte unchanged.
        #[tokio::test]
        async fn json_branch_skips_and_returns_verbatim() {
            let json = "{\"segments\": [{\"text\": \"Hello\"}]}";
            let mut complete = async |_: String| -> Result<String, Infallible> {
                panic!("JSON format must skip translation (no LLM call)");
            };
            let out =
                translate_content(json, "en", "es", OutputFormat::Json, 50, &mut complete).await;
            assert_eq!(out, json);
        }

        /// Same source==target: short-circuits before any LLM call, content
        /// unchanged.
        #[tokio::test]
        async fn same_language_short_circuits_verbatim() {
            let mut complete = async |_: String| -> Result<String, Infallible> {
                panic!("same-language must short-circuit (no LLM call)");
            };
            let out =
                translate_content(SRT_IN, "en", "en", OutputFormat::Srt, 50, &mut complete).await;
            assert_eq!(out, SRT_IN);
        }

        /// Empty content: short-circuits before any LLM call, empty out.
        #[tokio::test]
        async fn empty_content_short_circuits() {
            let mut complete = async |_: String| -> Result<String, Infallible> {
                panic!("empty content must short-circuit (no LLM call)");
            };
            let out = translate_content("", "en", "es", OutputFormat::Srt, 50, &mut complete).await;
            assert_eq!(out, "");
        }

        /// Exception fallback: a backend error during translation is swallowed
        /// and the original, untranslated content is returned verbatim.
        #[tokio::test]
        async fn translation_error_falls_back_to_original() {
            #[derive(Debug)]
            struct Boom;
            let mut complete = async |_: String| -> Result<String, Boom> { Err(Boom) };
            let out =
                translate_content(SRT_IN, "en", "es", OutputFormat::Srt, 50, &mut complete).await;
            assert_eq!(out, SRT_IN, "failed translation degrades to original");
        }
    }

    #[test]
    fn format_prompt_substitutes_three_placeholders() {
        let p = format_prompt(TRANSLATION_PROMPT, "en", "es", "Hello.");
        assert!(p.starts_with("Translate the following subtitle text from en to es."));
        assert!(p.ends_with("Text to translate:\nHello."));
    }

    /// Falsifier for the wire contract: each backend's outgoing request path,
    /// auth headers and JSON body are matched against an in-test golden under
    /// `wiremock`, and the reply is extracted from the provider's response shape.
    ///
    /// The three OpenAI-compatible providers (OpenAI/Ollama/Gemini) share the
    /// [`OpenAiCompatBackend`] driven by `async-openai`, so one chat-completions
    /// case covers the request shape they all emit; a second case pins Gemini's
    /// `/openai`-suffixed base to `…/openai/chat/completions`. Claude keeps its
    /// native messages-API shape.
    ///
    /// The goldens are written here rather than committed as fixtures because
    /// they assert the wire contract owned by this crate, not captured from the
    /// Python runtime.
    mod parity {
        use std::sync::mpsc;

        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, Request, ResponseTemplate};

        use super::super::{
            Backend, AnthropicBackend, OpenAiCompatBackend, DEFAULT_CLAUDE_MODEL, DEFAULT_OPENAI_MODEL,
        };

        /// Captured request: the JSON body plus the auth/version headers each
        /// provider is contractually required to send.
        struct Captured {
            body: serde_json::Value,
            headers: Vec<(String, String)>,
            reply: String,
        }

        /// Run `complete` against a wiremock server, capturing the request body
        /// and the named headers, returning them alongside the extracted reply.
        ///
        /// `build` receives the server's base URL (the `OpenAIConfig` API base
        /// or the Claude base URL), so a backend's own path suffix is appended
        /// on top of it; `request_path` is what the server should see.
        async fn run<B, F>(
            request_path: impl Into<String>,
            response: serde_json::Value,
            capture_headers: &'static [&'static str],
            build: F,
        ) -> Captured
        where
            B: Backend + Send + 'static,
            F: FnOnce(String) -> B + Send + 'static,
        {
            let request_path = request_path.into();
            let server = MockServer::start().await;

            let (tx, rx) = mpsc::channel::<(serde_json::Value, Vec<(String, String)>)>();
            Mock::given(method("POST"))
                .and(path(request_path))
                .respond_with(move |req: &Request| {
                    let body = req.body_json::<serde_json::Value>().unwrap();
                    let headers = capture_headers
                        .iter()
                        .filter_map(|name| {
                            req.headers
                                .get(*name)
                                .map(|v| (name.to_string(), v.to_str().unwrap().to_string()))
                        })
                        .collect::<Vec<_>>();
                    tx.send((body, headers)).unwrap();
                    ResponseTemplate::new(200).set_body_json(response.clone())
                })
                .mount(&server)
                .await;

            let backend = build(server.uri());
            let reply = backend.complete("Hello").await.unwrap();

            let (body, headers) = rx.recv().unwrap();
            Captured {
                body,
                headers,
                reply,
            }
        }

        /// The OpenAI-compatible reply payload all three providers return.
        ///
        /// `async-openai` deserializes the full `CreateChatCompletionResponse`,
        /// so the required envelope fields (`id`/`created`/`model`/`object`)
        /// must be present alongside `choices`.
        fn chat_response() -> serde_json::Value {
            serde_json::json!({
                "id": "chatcmpl-test",
                "object": "chat.completion",
                "created": 0,
                "model": "test-model",
                "choices": [{
                    "index": 0,
                    "message": {"role": "assistant", "content": "  hola  "},
                    "finish_reason": "stop",
                }],
            })
        }

        #[tokio::test]
        async fn openai_compat_payload() {
            // `async-openai` posts to `{base}/chat/completions` with Bearer auth
            // and a single user message; the reply is the stripped
            // choices[0].message.content.
            let captured = run(
                "/chat/completions",
                chat_response(),
                &["authorization"],
                |base| OpenAiCompatBackend::new("openai", "sk-test", DEFAULT_OPENAI_MODEL, base),
            )
            .await;

            assert_eq!(captured.reply, "hola");
            assert_eq!(captured.body["model"], DEFAULT_OPENAI_MODEL);
            assert_eq!(
                captured.body["messages"],
                serde_json::json!([{"role": "user", "content": "Hello"}])
            );
            assert_eq!(
                captured.headers,
                vec![("authorization".to_string(), "Bearer sk-test".to_string())]
            );
        }

        #[tokio::test]
        async fn gemini_base_appends_chat_completions() {
            // Gemini's base ends in `/openai`, so the crate's `url(path)` yields
            // `…/openai/chat/completions`. Pinning the full path here is the
            // verify-in-work guard that the OpenAI-compat shape reaches Gemini.
            let captured = run(
                "/v1beta/openai/chat/completions",
                chat_response(),
                &["authorization"],
                |base| {
                    OpenAiCompatBackend::new(
                        "gemini",
                        "g-test",
                        "gemini-2.5-flash",
                        format!("{base}/v1beta/openai"),
                    )
                },
            )
            .await;

            assert_eq!(captured.reply, "hola");
            assert_eq!(
                captured.headers,
                vec![("authorization".to_string(), "Bearer g-test".to_string())]
            );
        }

        #[tokio::test]
        async fn claude_payload() {
            // Claude: x-api-key + anthropic-version headers, max_tokens=4096,
            // single user message; reply from the first text block.
            let captured = run(
                "/v1/messages",
                serde_json::json!({
                    "content": [{"type": "text", "text": "  hola  "}],
                }),
                &["x-api-key", "anthropic-version"],
                |base| AnthropicBackend::with_base_url("sk-ant-test", DEFAULT_CLAUDE_MODEL, base),
            )
            .await;

            assert_eq!(captured.reply, "hola");
            assert_eq!(
                captured.body,
                serde_json::json!({
                    "model": DEFAULT_CLAUDE_MODEL,
                    "max_tokens": 4096,
                    "messages": [{"role": "user", "content": "Hello"}],
                })
            );
            assert_eq!(
                captured.headers,
                vec![
                    ("x-api-key".to_string(), "sk-ant-test".to_string()),
                    ("anthropic-version".to_string(), "2023-06-01".to_string()),
                ]
            );
        }
    }
}
