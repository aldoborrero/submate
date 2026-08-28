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
    /// ASR engine override from `?engine=`. `None` means the transcriber uses the
    /// server's configured default.
    pub engine: Option<submate_types::Engine>,
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
    /// Cap on Bazarr requests processed at once (`None` = unlimited). Bounds how
    /// many multi-hundred-MB uploads buffer in memory simultaneously.
    bazarr_concurrency: Option<usize>,
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

    /// Bound how many Bazarr requests are processed concurrently, capping the
    /// number of in-flight audio uploads buffered in memory. Typically the
    /// transcription runner count.
    pub fn with_bazarr_concurrency(mut self, max: usize) -> Self {
        self.bazarr_concurrency = Some(max);
        self
    }
}

/// Build the server [`Router`]: always the ops routes, plus the bazarr router
/// when the `bazarr` feature is on (the default).
pub fn app(state: AppState) -> Router {
    let router = ops_router();

    // Bound concurrent Bazarr requests (only these buffer large uploads) with a
    // *global* limit shared across connections, so peak upload memory is capped
    // at `bazarr_concurrency × body-limit` instead of unbounded. Excess requests
    // wait — fine for Bazarr's synchronous long-poll. Ops routes stay unlimited.
    #[cfg(feature = "bazarr")]
    let router = {
        let bazarr = match state.bazarr_concurrency {
            Some(n) if n > 0 => {
                bazarr_router().layer(tower::limit::GlobalConcurrencyLimitLayer::new(n))
            }
            _ => bazarr_router(),
        };
        router.merge(bazarr)
    };

    // Bazarr uploads a full episode's extracted audio (16 kHz mono PCM, tens to
    // hundreds of MB) as a multipart `audio_file`, far past axum's 2 MB default
    // body limit. The limit must be applied here, on the merged router: a
    // `DefaultBodyLimit` layered onto a sub-router *before* `.merge()` does not
    // take effect, so large uploads were silently truncated and the handler
    // returned an empty subtitle ("Completed in 0:00:00" in Bazarr).
    router
        .layer(DefaultBodyLimit::max(1024 * 1024 * 1024))
        .with_state(state)
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
    // NOTE: the multipart body-size limit for these upload routes is applied in
    // `app()`, on the merged router — a `DefaultBodyLimit` layered here (before
    // the `.merge()` in `app()`) does not take effect in axum 0.7.
    Router::new()
        .route("/bazarr/asr", post(bazarr_asr))
        .route("/bazarr/detect-language", post(bazarr_detect_language))
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
    /// ASR engine override (`whisper`/`parakeet`). Typed as a lenient `String` so a
    /// bad value never trips axum's `422`; mapped by hand via `parse_engine_param`.
    /// Absent/unknown falls back to the server's configured default.
    #[serde(default)]
    engine: Option<String>,
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

/// Read the `audio_file` multipart field (Bazarr's raw s16le PCM).
///
/// Distinguishes three outcomes so a broken transfer is never mistaken for a
/// valid empty request: `Ok(Some(bytes))` on success, `Ok(None)` when the field
/// is cleanly absent, and `Err(())` when the multipart stream or the field body
/// errored (e.g. a truncated upload or a tripped body limit). A read error must
/// not be transcribed as if it were complete audio — the caller signals failure
/// so Bazarr retries instead of saving a wrong/empty subtitle.
#[cfg(feature = "bazarr")]
async fn read_audio_file(mut multipart: Multipart) -> Result<Option<Vec<u8>>, ()> {
    loop {
        match multipart.next_field().await {
            Ok(Some(field)) => {
                if field.name() == Some("audio_file") {
                    return field
                        .bytes()
                        .await
                        .map(|b| Some(b.to_vec()))
                        .map_err(|_| ());
                }
            }
            Ok(None) => return Ok(None),
            Err(_) => return Err(()),
        }
    }
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

/// Map the `?engine=` query value to an [`Engine`], case-insensitively. Unknown/absent →
/// `None` (the caller uses the server's configured default). Never rejects the request.
#[cfg(feature = "bazarr")]
fn parse_engine_param(engine: Option<&str>) -> Option<submate_types::Engine> {
    engine?.to_ascii_lowercase().parse().ok()
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
    let pcm = match read_audio_file(multipart).await {
        Ok(Some(pcm)) => pcm,
        // No audio_file field: a malformed request with nothing to transcribe.
        Ok(None) => return empty_asr_response(),
        // Truncated/broken upload: return the empty body (Bazarr discards it and
        // retries on its next scan) but log it, rather than silently transcribing
        // a partial clip. The route contract is "empty body, never an error
        // envelope", so no 5xx even here.
        Err(()) => {
            tracing::warn!("bazarr asr: audio_file upload failed mid-read; returning empty");
            return empty_asr_response();
        }
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
        engine: parse_engine_param(params.engine.as_deref()),
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
    // Detection failing (absent or broken upload) degrades to "unknown" rather
    // than erroring — a wrong language guess isn't saved as a subtitle.
    let Ok(Some(pcm)) = read_audio_file(multipart).await else {
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

    /// A seam that records what it was handed — the PCM (to prove the multipart
    /// `audio_file` part reaches the transcriber byte-for-byte, raw s16le, not
    /// WAV-wrapped or otherwise mangled) and the resolved `opts.engine` (to prove
    /// the `?engine=` query param threads through to the seam).
    #[derive(Default)]
    struct Recorder {
        pcm: Arc<Mutex<Vec<u8>>>,
        engine: Arc<Mutex<Option<submate_types::Engine>>>,
    }

    #[async_trait::async_trait]
    impl BazarrTranscriber for Recorder {
        async fn transcribe(
            &self,
            opts: BazarrTranscribeOpts,
            pcm: Vec<u8>,
        ) -> std::result::Result<BazarrOutput, String> {
            *self.pcm.lock().unwrap() = pcm;
            *self.engine.lock().unwrap() = opts.engine;
            Ok(BazarrOutput {
                content: SRT.to_string(),
                detected_language: "es".to_string(),
            })
        }

        async fn detect(&self, _pcm: Vec<u8>) -> std::result::Result<BazarrDetected, String> {
            Err("unused".to_string())
        }
    }

    #[test]
    fn parse_engine_param_is_lenient() {
        use submate_types::Engine;
        assert_eq!(parse_engine_param(None), None);
        assert_eq!(parse_engine_param(Some("whisper")), Some(Engine::Whisper));
        assert_eq!(parse_engine_param(Some("Parakeet")), Some(Engine::Parakeet));
        assert_eq!(parse_engine_param(Some("garbage")), None);
    }

    #[tokio::test]
    async fn asr_passes_raw_pcm_unwrapped() {
        let rec = Recorder::default();
        let seen = rec.pcm.clone();
        let state = AppState::default().with_bazarr(Arc::new(rec));
        let pcm = vec![0xde, 0xad, 0xbe, 0xef, 0x01, 0x02];
        let _ = post(state, "/bazarr/asr?output=srt&encode=false", &pcm).await;
        assert_eq!(
            *seen.lock().unwrap(),
            pcm,
            "the seam must receive the exact uploaded PCM, unwrapped"
        );
    }

    /// End-to-end wiring: `?engine=` on `POST /bazarr/asr` must reach the seam as
    /// `opts.engine`. A known value maps to `Some(engine)`; a garbage value stays
    /// lenient (`None` → server default, empty body, no 4xx/5xx).
    #[tokio::test]
    async fn asr_engine_param_reaches_seam() {
        use submate_types::Engine;

        // Known engine threads through to the seam.
        let rec = Recorder::default();
        let engine = rec.engine.clone();
        let state = AppState::default().with_bazarr(Arc::new(rec));
        let (status, _h, _body) =
            post(state, "/bazarr/asr?engine=parakeet&output=srt", b"\x00\x01").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(*engine.lock().unwrap(), Some(Engine::Parakeet));

        // Garbage engine → None (lenient default), never a 4xx/5xx.
        let rec = Recorder::default();
        let engine = rec.engine.clone();
        let state = AppState::default().with_bazarr(Arc::new(rec));
        let (status, _h, _body) =
            post(state, "/bazarr/asr?engine=garbage&output=srt", b"\x00\x01").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(*engine.lock().unwrap(), None);
    }
}
