//! axum server: bazarr + ops routes.
//!
//! This crate builds the [`Router`] for the submate server. The **ops routes**
//! (`/`, `/status`) are always present; the **bazarr** integration router (the
//! Whisper ASR provider) is feature-flagged (on by default) and runs a direct,
//! semaphore-bounded transcription via the [`BazarrTranscriber`] seam.

use std::sync::Arc;

use axum::{
    Json, Router,
    body::Body,
    extract::{DefaultBodyLimit, Multipart, Query, State},
    http::{HeaderName, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::Deserialize;
use serde_json::json;
use submate_types::{OutputFormat, TranscriptionTask};

/// Server version reported by the ops routes.
///
/// This is the user-facing product version, intentionally distinct from the
/// Rust workspace crate version. The two version lines move independently.
pub const VERSION: &str = "1.0.0";

/// A subtitle produced for a Bazarr request, plus the language Whisper detected.
pub struct BazarrOutput {
    /// The rendered subtitle text (SRT/VTT/TXT/JSON), already translated to the
    /// target language if one was requested and differed from the detected one.
    pub content: String,
    /// The source language Whisper detected (ISO-639-1), used to decide whether
    /// translation was needed.
    pub detected_language: String,
}

/// The `{detected_language, language_code}` pair the detect-language route emits.
pub struct BazarrDetected {
    /// Human-readable display name (or `"Unknown"`).
    pub detected_language: String,
    /// The normalized language code (or `"und"`).
    pub language_code: String,
}

/// Parameters for one direct Bazarr transcription.
pub struct BazarrTranscribeOpts {
    /// `transcribe` (source language) or `translate` (Whisper → English).
    pub task: TranscriptionTask,
    /// Desired subtitle language. Bazarr sends this as `language`; when it
    /// differs from the detected source, the transcriber LLM-translates to it.
    /// Source language is always auto-detected.
    pub target_language: Option<String>,
    /// Subtitle format to render.
    pub output_format: OutputFormat,
    /// Emit word-level timestamps in SRT/VTT.
    pub word_timestamps: bool,
}

/// The synchronous, semaphore-bounded Bazarr transcription seam.
///
/// Bazarr's Whisper provider is a *synchronous* RPC — it holds the connection
/// per file and reads the subtitle from the response body — so the Bazarr routes
/// run a transcription directly via this seam. The production impl (built in
/// `cmd_server`) wraps a [`submate_whisper::Dispatcher`] so concurrent Bazarr
/// requests share a runner cap; tests inject a fake. The permit is acquired
/// *inside* `transcribe`, so a busy server waits for a runner rather than
/// failing — Bazarr's transcription timeout is large by design.
#[async_trait::async_trait]
pub trait BazarrTranscriber: Send + Sync {
    /// Transcribe `pcm` (raw s16le/mono/16k) into the requested subtitle format,
    /// translating to `opts.target_language` when it differs from the detected
    /// source. `Err(msg)` on any failure — the route renders that as an **empty**
    /// response body, never an error envelope (Bazarr saves the body verbatim).
    async fn transcribe(
        &self,
        opts: BazarrTranscribeOpts,
        pcm: Vec<u8>,
    ) -> std::result::Result<BazarrOutput, String>;

    /// Detect the spoken language of `pcm`, returning the display-name/code pair.
    /// `Err(_)` becomes the `{"Unknown","und"}` 200 envelope at the route.
    async fn detect(&self, pcm: Vec<u8>) -> std::result::Result<BazarrDetected, String>;
}

/// Shared application state handed to the route handlers.
///
/// Holds the optional Bazarr transcription seam; without it the `/bazarr/*`
/// routes degrade gracefully (empty body / `Unknown`).
#[derive(Clone, Default)]
pub struct AppState {
    bazarr: Option<Arc<dyn BazarrTranscriber>>,
}

impl AppState {
    /// A server with no transcription seam wired up (the `/bazarr/*` routes
    /// degrade gracefully until [`AppState::with_bazarr`] attaches one).
    pub fn new() -> Self {
        Self::default()
    }

    /// Attach the direct Bazarr transcription seam.
    pub fn with_bazarr(mut self, bazarr: Arc<dyn BazarrTranscriber>) -> Self {
        self.bazarr = Some(bazarr);
        self
    }
}

/// Build the server [`Router`]: always the ops routes, plus the bazarr router
/// when the `bazarr` feature is on (the default).
pub fn app(state: AppState) -> Router {
    let router = ops_router();

    #[cfg(feature = "bazarr")]
    let router = router.merge(bazarr_router());

    router.with_state(state)
}

/// The ops routes.
fn ops_router() -> Router<AppState> {
    Router::new()
        .route("/", get(root))
        .route("/status", get(status))
}

/// `GET /` — server-info object.
async fn root() -> Json<serde_json::Value> {
    Json(json!({
        "name": "Submate Server",
        "version": VERSION,
        "docs": "/docs",
        "endpoints": {
            "bazarr_asr": "/bazarr/asr",
            "bazarr_detect_language": "/bazarr/detect-language",
            "status": "/status",
        },
    }))
}

/// `GET /status` — health + version.
async fn status() -> Json<serde_json::Value> {
    Json(json!({
        "status": "ok",
        "version": VERSION,
    }))
}

/// The Bazarr routes (`POST /bazarr/asr`, `POST /bazarr/detect-language`). They
/// run a direct transcription via the [`BazarrTranscriber`] seam.
#[cfg(feature = "bazarr")]
fn bazarr_router() -> Router<AppState> {
    Router::new()
        .route("/bazarr/asr", post(bazarr_asr))
        .route("/bazarr/detect-language", post(bazarr_detect_language))
        // Bazarr uploads the whole extracted audio stream (16 kHz mono PCM) as a
        // multipart `audio_file`. A full episode/movie is tens to hundreds of MB,
        // far past axum's 2 MB default body limit — without this, large uploads
        // are rejected and the handler sees no audio, returning an empty subtitle
        // instantly (Bazarr logs "Completed in 0:00:00" + "subtitles isn't valid").
        .layer(DefaultBodyLimit::max(1024 * 1024 * 1024))
}

/// `Source` response header the `/bazarr/asr` handler sets.
#[cfg(feature = "bazarr")]
const BAZARR_SOURCE: &str = "Transcribed using stable-ts from Submate";

/// `POST /bazarr/asr` query params.
///
/// Fields are typed leniently (optional / string) so a well-formed Bazarr
/// request never trips axum's `422` query-rejection — Bazarr reads the body
/// verbatim and would save a `422` envelope as a corrupt subtitle.
#[cfg(feature = "bazarr")]
#[derive(Deserialize)]
struct AsrParams {
    #[serde(default = "default_task")]
    task: String,
    /// Desired subtitle language (Bazarr's `language`) — the *target*, not a
    /// Whisper decode hint; source is auto-detected.
    #[serde(default)]
    language: Option<String>,
    #[serde(default = "default_output")]
    output: String,
    /// Accepted but ignored (Bazarr sends `encode=false` after pre-encoding).
    #[serde(default)]
    #[expect(dead_code)]
    encode: Option<String>,
    #[serde(default)]
    word_timestamps: bool,
    #[serde(default)]
    #[expect(dead_code)]
    video_file: Option<String>,
}

#[cfg(feature = "bazarr")]
fn default_task() -> String {
    "transcribe".to_string()
}
#[cfg(feature = "bazarr")]
fn default_output() -> String {
    "srt".to_string()
}

/// `POST /bazarr/detect-language` query params. All accepted, all ignored: the
/// real provider sends no offset/length and we detect on the uploaded clip.
#[cfg(feature = "bazarr")]
#[derive(Deserialize)]
#[expect(dead_code)]
struct DetectParams {
    #[serde(default)]
    encode: Option<String>,
    #[serde(default)]
    detect_lang_length: Option<u32>,
    #[serde(default)]
    detect_lang_offset: Option<u32>,
    #[serde(default)]
    video_file: Option<String>,
}

/// Read the `audio_file` multipart field (Bazarr's raw s16le PCM), or `None` if
/// the part is absent or unreadable.
#[cfg(feature = "bazarr")]
async fn read_audio_file(mut multipart: Multipart) -> Option<Vec<u8>> {
    while let Ok(Some(field)) = multipart.next_field().await {
        if field.name() == Some("audio_file") {
            return field.bytes().await.ok().map(|b| b.to_vec());
        }
    }
    None
}

/// Map Bazarr's `output` value to an [`OutputFormat`] (Bazarr always sends
/// `srt`; the rest are accepted for non-Bazarr clients).
#[cfg(feature = "bazarr")]
fn parse_output_format(output: &str) -> Option<OutputFormat> {
    match output {
        "srt" => Some(OutputFormat::Srt),
        "vtt" => Some(OutputFormat::Vtt),
        "txt" => Some(OutputFormat::Txt),
        "json" => Some(OutputFormat::Json),
        "ass" => Some(OutputFormat::Ass),
        _ => None,
    }
}

/// A `200` response with an empty `text/plain` body — the only safe failure
/// signal for `/asr` (the provider saves `r.content` with no status check, so an
/// error body would become a corrupt subtitle; an empty body is discarded and
/// Bazarr retries on its schedule).
#[cfg(feature = "bazarr")]
fn empty_asr_response() -> Response {
    ([(header::CONTENT_TYPE, "text/plain")], Body::empty()).into_response()
}

/// `POST /bazarr/asr` — direct, semaphore-bounded transcription. Returns the
/// subtitle as the response body with the `Source` header on success, and an
/// **empty body** on any failure (see [`empty_asr_response`]).
#[cfg(feature = "bazarr")]
#[tracing::instrument(
    name = "bazarr_asr",
    skip_all,
    fields(task = %params.task, output = %params.output)
)]
async fn bazarr_asr(
    State(state): State<AppState>,
    Query(params): Query<AsrParams>,
    multipart: Multipart,
) -> Response {
    let Some(bazarr) = state.bazarr.clone() else {
        return empty_asr_response();
    };
    let Some(pcm) = read_audio_file(multipart).await else {
        return empty_asr_response();
    };
    tracing::debug!(pcm_bytes = pcm.len(), "received asr request");
    let Some(output_format) = parse_output_format(&params.output) else {
        return empty_asr_response();
    };
    let task = match params.task.as_str() {
        "translate" => TranscriptionTask::Translate,
        _ => TranscriptionTask::Transcribe,
    };
    let opts = BazarrTranscribeOpts {
        task,
        target_language: params.language,
        output_format,
        word_timestamps: params.word_timestamps,
    };
    match bazarr.transcribe(opts, pcm).await {
        Ok(out) => (
            [
                (header::CONTENT_TYPE, "text/plain"),
                (HeaderName::from_static("source"), BAZARR_SOURCE),
            ],
            out.content,
        )
            .into_response(),
        Err(err) => {
            tracing::warn!(error = %err, "bazarr asr failed; returning empty body");
            empty_asr_response()
        }
    }
}

/// `POST /bazarr/detect-language` — always `200`. Returns
/// `{detected_language, language_code}` on success and the `{"Unknown","und"}`
/// envelope on any failure (Bazarr maps a non-conforming reply to "undetected").
#[cfg(feature = "bazarr")]
#[tracing::instrument(name = "bazarr_detect_language", skip_all)]
async fn bazarr_detect_language(
    State(state): State<AppState>,
    Query(_params): Query<DetectParams>,
    multipart: Multipart,
) -> Json<serde_json::Value> {
    let unknown = || json!({ "detected_language": "Unknown", "language_code": "und" });
    let Some(bazarr) = state.bazarr.clone() else {
        return Json(unknown());
    };
    let Some(pcm) = read_audio_file(multipart).await else {
        return Json(unknown());
    };
    match bazarr.detect(pcm).await {
        Ok(d) => Json(json!({
            "detected_language": d.detected_language,
            "language_code": d.language_code,
        })),
        Err(err) => {
            tracing::debug!(error = %err, "bazarr detect-language failed; returning Unknown");
            Json(unknown())
        }
    }
}

#[cfg(test)]
mod ops_tests {
    use super::*;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    async fn get_json(uri: &str) -> (StatusCode, serde_json::Value) {
        let resp = app(AppState::default())
            .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = resp.status();
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let value = if bytes.is_empty() {
            serde_json::Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap()
        };
        (status, value)
    }

    #[tokio::test]
    async fn status_reports_ok_and_version() {
        let (status, body) = get_json("/status").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["status"], "ok");
        assert_eq!(body["version"], VERSION);
        // The queue snapshot is gone with the durable queue.
        assert!(body.get("queue").is_none());
    }

    #[tokio::test]
    async fn unknown_route_is_not_found() {
        let (status, _body) = get_json("/nope").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }
}

/// Bazarr Whisper-provider contract tests — drive `app()` with a fake
/// [`BazarrTranscriber`] (no model) and pin the behaviors Bazarr's provider
/// depends on: SRT-in-body + `Source` header, an **empty body** on failure
/// (never an error envelope, which the provider would save as a corrupt
/// subtitle), and detect-language as `200` JSON / `200`-`Unknown` on failure.
#[cfg(all(test, feature = "bazarr"))]
mod bazarr_routes_tests {
    use super::*;
    use axum::http::{HeaderMap, Request, StatusCode};
    use http_body_util::BodyExt;
    use std::sync::Mutex;
    use tower::ServiceExt;

    const SRT: &str = "1\n00:00:00,000 --> 00:00:01,000\nhola\n";

    /// Configurable fake seam: succeed with canned output, or fail.
    struct Fake {
        fail: bool,
    }

    #[async_trait::async_trait]
    impl BazarrTranscriber for Fake {
        async fn transcribe(
            &self,
            _opts: BazarrTranscribeOpts,
            _pcm: Vec<u8>,
        ) -> std::result::Result<BazarrOutput, String> {
            if self.fail {
                Err("boom".to_string())
            } else {
                Ok(BazarrOutput {
                    content: SRT.to_string(),
                    detected_language: "es".to_string(),
                })
            }
        }

        async fn detect(&self, _pcm: Vec<u8>) -> std::result::Result<BazarrDetected, String> {
            if self.fail {
                Err("boom".to_string())
            } else {
                Ok(BazarrDetected {
                    detected_language: "Spanish".to_string(),
                    language_code: "es".to_string(),
                })
            }
        }
    }

    /// Build a `multipart/form-data` body with the `audio_file` part (raw PCM).
    fn multipart(pcm: &[u8]) -> (String, Vec<u8>) {
        let boundary = "submateBazarrBoundary";
        let mut body = Vec::new();
        body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
        body.extend_from_slice(
            b"Content-Disposition: form-data; name=\"audio_file\"; filename=\"audio.pcm\"\r\n",
        );
        body.extend_from_slice(b"Content-Type: application/octet-stream\r\n\r\n");
        body.extend_from_slice(pcm);
        body.extend_from_slice(b"\r\n");
        body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
        (format!("multipart/form-data; boundary={boundary}"), body)
    }

    fn with_fake(fail: bool) -> AppState {
        AppState::default().with_bazarr(Arc::new(Fake { fail }))
    }

    async fn post(state: AppState, uri: &str, pcm: &[u8]) -> (StatusCode, HeaderMap, Vec<u8>) {
        let (content_type, body) = multipart(pcm);
        let resp = app(state)
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(uri)
                    .header(header::CONTENT_TYPE, content_type)
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = resp.status();
        let headers = resp.headers().clone();
        let bytes = resp
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .to_vec();
        (status, headers, bytes)
    }

    #[tokio::test]
    async fn asr_returns_srt_body_with_source_header() {
        let (status, headers, body) = post(
            with_fake(false),
            "/bazarr/asr?task=transcribe&language=es&output=srt&encode=false",
            b"\x00\x01\x02\x03",
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            headers.get("source").unwrap(),
            "Transcribed using stable-ts from Submate"
        );
        assert!(
            headers
                .get(header::CONTENT_TYPE)
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("text/plain")
        );
        assert_eq!(String::from_utf8(body).unwrap(), SRT);
    }

    #[tokio::test]
    async fn asr_failure_returns_empty_body() {
        // Transcriber error → empty body, never an error envelope.
        let (status, _h, body) = post(
            with_fake(true),
            "/bazarr/asr?output=srt&encode=false",
            b"\x00\x01",
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert!(
            body.is_empty(),
            "failure must be an empty body, got {body:?}"
        );

        // No seam wired (brain-only server) → also an empty body.
        let (status, _h, body) =
            post(AppState::default(), "/bazarr/asr?output=srt", b"\x00\x01").await;
        assert_eq!(status, StatusCode::OK);
        assert!(body.is_empty());
    }

    #[tokio::test]
    async fn detect_returns_json() {
        let (status, _h, body) = post(
            with_fake(false),
            "/bazarr/detect-language?encode=false",
            b"\x00\x01\x02\x03",
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(v["detected_language"], "Spanish");
        assert_eq!(v["language_code"], "es");
    }

    #[tokio::test]
    async fn detect_failure_is_200_unknown() {
        let (status, _h, body) =
            post(with_fake(true), "/bazarr/detect-language", b"\x00\x01").await;
        assert_eq!(status, StatusCode::OK);
        let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(v["detected_language"], "Unknown");
        assert_eq!(v["language_code"], "und");
    }

    /// A seam that records the PCM it was handed, to prove the multipart
    /// `audio_file` part reaches the transcriber byte-for-byte (raw s16le, not
    /// WAV-wrapped or otherwise mangled).
    struct Recorder(Arc<Mutex<Vec<u8>>>);

    #[async_trait::async_trait]
    impl BazarrTranscriber for Recorder {
        async fn transcribe(
            &self,
            _opts: BazarrTranscribeOpts,
            pcm: Vec<u8>,
        ) -> std::result::Result<BazarrOutput, String> {
            *self.0.lock().unwrap() = pcm;
            Ok(BazarrOutput {
                content: SRT.to_string(),
                detected_language: "es".to_string(),
            })
        }

        async fn detect(&self, _pcm: Vec<u8>) -> std::result::Result<BazarrDetected, String> {
            Err("unused".to_string())
        }
    }

    #[tokio::test]
    async fn asr_passes_raw_pcm_unwrapped() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let state = AppState::default().with_bazarr(Arc::new(Recorder(seen.clone())));
        let pcm = vec![0xde, 0xad, 0xbe, 0xef, 0x01, 0x02];
        let _ = post(state, "/bazarr/asr?output=srt&encode=false", &pcm).await;
        assert_eq!(
            *seen.lock().unwrap(),
            pcm,
            "the seam must receive the exact uploaded PCM, unwrapped"
        );
    }
}
