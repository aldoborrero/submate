//! axum server: bazarr + jellyfin + ops routes (ports `submate/server/`).
//!
//! This crate builds the [`Router`] for the submate server. The **ops routes**
//! mirror `submate/server/handlers/core/router.py` (`/`, `/status`, `/queue`)
//! and are always present. The integration routers (bazarr, jellyfin) are
//! feature-flagged and mounted only when their feature is enabled; they are
//! filled in by later backlog items.
//!
//! ## Topology note
//!
//! The queue-stats *shape* (`pending` / `running` / `done` / `nodes`) follows
//! the FileFlows/Unmanic-style node topology described in
//! `rust/docs/architecture.md`, **not** Huey's `pending` / `scheduled`. It is a
//! new design verified behaviorally, so it is not a Python-golden parity case.
//! Python's top-level *route names and response keys* are matched exactly.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use axum::{
    body::Body,
    extract::{Multipart, Path, Query, State},
    http::{header, HeaderName, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use submate_config::ServerSettings;
use submate_media::{prepare_audio_for_transcription, PreparedAudio};
use submate_node::{Agent, AgentError, Dispatcher, JobProcessor};
use submate_proto::{
    JobOpts, JobOutcome, JobResult, NodeRegister, NodeRegistered, OutputFormat, Progress,
    WorkResponse,
};
use submate_queue::{JobId, JobState, JobStore, NewJob, QueueError};
use submate_types::TranscriptionTask;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

/// Server version reported by the ops routes.
///
/// This mirrors Python's `submate.__version__` (the user-facing product
/// version), which is intentionally distinct from the Rust workspace crate
/// version. The two version lines move independently.
pub const VERSION: &str = "1.0.0";

/// Node-topology queue statistics surfaced by `GET /queue` and embedded in
/// `GET /status`.
///
/// The shape follows the server↔node coordination model (see
/// `rust/docs/architecture.md`): job counts by lifecycle state plus the number
/// of connected processing nodes. On an empty/idle server every field is `0`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct QueueStats {
    /// Jobs waiting to be claimed.
    pub pending: u64,
    /// Jobs currently leased to a node.
    pub running: u64,
    /// Jobs that completed successfully.
    pub done: u64,
    /// Processing nodes currently registered/connected.
    pub nodes: u64,
}

/// Source of live [`QueueStats`] for the ops routes.
///
/// The server owns the durable queue and the node registry; this trait lets the
/// ops routes read a snapshot without depending on those concretes, which are
/// wired in by later backlog items. The default state (no queue/registry yet)
/// reports zeroed counts.
pub trait StatsSource: Send + Sync + 'static {
    /// Take a current snapshot of the queue/node statistics.
    fn stats(&self) -> QueueStats;
}

/// Default [`StatsSource`] for a server with no queue or nodes wired up yet:
/// always reports zeroed counts.
#[derive(Debug, Clone, Copy, Default)]
pub struct EmptyStats;

impl StatsSource for EmptyStats {
    fn stats(&self) -> QueueStats {
        QueueStats::default()
    }
}

/// Shared application state handed to the route handlers.
///
/// `stats` always backs the ops routes. `coord`, when present, is the live
/// node-coordination core (durable queue + node registry) that the
/// `/nodes/*` and `/jobs/*` routes operate on; a coordinator also serves as the
/// `stats` source so `GET /queue` reflects real job/node counts.
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
    /// Source language is always auto-detected (mirrors the Python handler).
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
/// run a transcription **directly** rather than through the durable queue (which
/// is for the async, file-backed scan/Jellyfin paths). The production impl
/// (built in `cmd_server`) wraps the embedded node's [`Dispatcher`], so Bazarr
/// shares the runner cap with the queue drain; tests inject a fake. The permit
/// is acquired *inside* `transcribe`, so a busy server waits for a runner rather
/// than failing — Bazarr's transcription timeout is large by design.
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

#[derive(Clone)]
pub struct AppState {
    stats: Arc<dyn StatsSource>,
    coord: Option<Arc<NodeCoordinator>>,
    server: Arc<ServerSettings>,
    bazarr: Option<Arc<dyn BazarrTranscriber>>,
}

impl AppState {
    /// Build state from a [`StatsSource`] with no node coordinator wired up.
    /// The `/nodes/*` and `/jobs/*` routes return `503` until a coordinator is
    /// attached via [`AppState::with_coordinator`]. Server processing settings
    /// default to the Python defaults (`process_on_add = true`).
    pub fn new(stats: impl StatsSource) -> Self {
        Self {
            stats: Arc::new(stats),
            coord: None,
            server: Arc::new(ServerSettings::default()),
            bazarr: None,
        }
    }

    /// Build state backed by a live [`NodeCoordinator`]. The coordinator also
    /// supplies live [`QueueStats`] for the ops routes.
    pub fn with_coordinator(coord: Arc<NodeCoordinator>) -> Self {
        Self {
            stats: coord.clone(),
            coord: Some(coord),
            server: Arc::new(ServerSettings::default()),
            bazarr: None,
        }
    }

    /// Override the server processing settings (`process_on_add` /
    /// `process_on_play`) that gate Jellyfin webhook handling.
    pub fn with_server_settings(mut self, server: ServerSettings) -> Self {
        self.server = Arc::new(server);
        self
    }

    /// Attach the direct Bazarr transcription seam. Without it the `/bazarr/*`
    /// routes degrade gracefully (empty body / `Unknown`), so a brain-only
    /// server stays Bazarr-safe.
    pub fn with_bazarr(mut self, bazarr: Arc<dyn BazarrTranscriber>) -> Self {
        self.bazarr = Some(bazarr);
        self
    }

    fn coordinator(&self) -> std::result::Result<&Arc<NodeCoordinator>, ServerError> {
        self.coord
            .as_ref()
            .ok_or_else(|| ServerError::Unavailable("node coordination not enabled".to_string()))
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new(EmptyStats)
    }
}

/// Errors surfaced by the server, rendered by the global error handler.
///
/// Every variant maps to a JSON body `{"detail": "<message>"}` plus an HTTP
/// status, matching FastAPI's `HTTPException` envelope, so clients see a single,
/// predictable error envelope regardless of which handler failed.
#[derive(Debug, thiserror::Error)]
pub enum ServerError {
    /// A requested resource does not exist.
    #[error("{0}")]
    NotFound(String),
    /// The request was malformed or failed validation.
    #[error("{0}")]
    BadRequest(String),
    /// An unexpected internal failure.
    #[error("{0}")]
    Internal(String),
    /// A required subsystem (e.g. node coordination) is not wired up.
    #[error("{0}")]
    Unavailable(String),
}

impl ServerError {
    fn status(&self) -> StatusCode {
        match self {
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::Unavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
        }
    }
}

impl From<QueueError> for ServerError {
    fn from(err: QueueError) -> Self {
        match err {
            QueueError::NotFound(id) => Self::NotFound(format!("job {id} not found")),
            other => Self::Internal(other.to_string()),
        }
    }
}

impl IntoResponse for ServerError {
    fn into_response(self) -> Response {
        let status = self.status();
        if status.is_server_error() {
            tracing::error!(error = %self, "request failed");
        }
        (status, Json(json!({ "detail": self.to_string() }))).into_response()
    }
}

/// Where a job's audio comes from, so `GET /jobs/{id}/audio` can serve it.
///
/// The server is the only place audio is produced (nodes have no media access),
/// so each enqueued job records how to materialise its s16le/mono/16k PCM:
///
/// - [`AudioSource::File`] — a media file the server extracts on demand (the
///   Jellyfin / local-scan path). The optional language hint picks the audio
///   track when a file has several, matching the transcription pipeline.
/// - [`AudioSource::Pcm`] — already-decoded PCM the server only relays (the
///   Bazarr path, where the caller uploaded the audio). Held behind an `Arc` so
///   the bytes are shared, not copied, on each fetch.
#[derive(Debug, Clone)]
pub enum AudioSource {
    /// A media file on the server; its audio is extracted when first fetched.
    File {
        /// Path to the media file the server can read.
        path: PathBuf,
        /// Language hint used to choose the audio track on multi-track files.
        language: Option<String>,
    },
    /// Already-decoded s16le/mono/16k PCM the server relays verbatim.
    Pcm(Arc<Vec<u8>>),
}

/// What this server knows about a registered processing node.
///
/// The token authorises the node's subsequent coordination calls; `runners` and
/// `gpu`/`tasks` are the advertised capabilities the dispatcher uses for routing
/// (capability filtering is refined by later backlog items). The registry is the
/// `nodes` count surfaced by `GET /queue`.
#[derive(Debug, Clone)]
struct NodeInfo {
    token: String,
    gpu: bool,
    runners: u32,
    tasks: Vec<TranscriptionTask>,
}

/// The pull-based node-coordination core: the durable [`JobStore`] plus the live
/// node registry and the set of synchronous result-waiters.
///
/// This is the server side of the FileFlows/Unmanic topology in
/// `rust/docs/architecture.md`: nodes register, long-poll [`request_work`] for an
/// atomically-claimed job, stream [`progress`], post a terminal [`result`], and
/// [`heartbeat`] to keep their lease alive. A caller that enqueued a job
/// synchronously (e.g. the Bazarr ASR route) can park on
/// [`wait_for_result`](NodeCoordinator::wait_for_result) and is woken when the
/// matching `result` arrives.
///
/// The [`JobStore`] is not internally synchronised, so it lives behind a
/// `Mutex`; the registry and waiter map have their own locks. Locks are only
/// ever held for the duration of a single store/registry operation, never across
/// an `.await`, so the long-poll cannot stall other requests.
///
/// [`request_work`]: NodeCoordinator::request_work
/// [`progress`]: NodeCoordinator::progress
/// [`result`]: NodeCoordinator::result
/// [`heartbeat`]: NodeCoordinator::heartbeat
pub struct NodeCoordinator {
    store: Mutex<JobStore>,
    nodes: Mutex<HashMap<String, NodeInfo>>,
    waiters: Mutex<HashMap<JobId, oneshot::Sender<JobOutcome>>>,
    /// Per-job progress subscribers, parallel to `waiters`. A synchronous caller
    /// (e.g. `transcribe --sync`) registers an unbounded channel via
    /// [`subscribe_progress`](NodeCoordinator::subscribe_progress) and
    /// [`progress`](NodeCoordinator::progress) fans each in-flight update out to
    /// it in arrival order. Unbounded so a node's `progress` POST never blocks on
    /// a slow renderer; a dropped receiver simply discards later updates.
    progress_subs: Mutex<HashMap<JobId, mpsc::UnboundedSender<Progress>>>,
    /// Per-job audio source, populated at enqueue time, that
    /// `GET /jobs/{id}/audio` materialises into PCM. Jobs enqueued without a
    /// source (e.g. via the legacy [`enqueue`](NodeCoordinator::enqueue)) have
    /// no entry here and their audio route reports `404`.
    audio: Mutex<HashMap<JobId, AudioSource>>,
    /// Base URL path nodes use to fetch a job's extracted audio (the
    /// `audio_url` in a [`WorkResponse::Work`] is `{audio_base}/{job_id}/audio`).
    audio_base: String,
}

impl NodeCoordinator {
    /// Build a coordinator over an existing job store.
    pub fn new(store: JobStore) -> Self {
        Self {
            store: Mutex::new(store),
            nodes: Mutex::new(HashMap::new()),
            waiters: Mutex::new(HashMap::new()),
            progress_subs: Mutex::new(HashMap::new()),
            audio: Mutex::new(HashMap::new()),
            audio_base: "/jobs".to_string(),
        }
    }

    fn store(&self) -> std::sync::MutexGuard<'_, JobStore> {
        // Poisoning only happens if a holder panicked mid-operation; the store
        // is a plain SQLite wrapper with no broken-invariant risk, so recover
        // the guard rather than propagating the panic to every later request.
        self.store.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Enqueue a job for `kind` carrying serialized [`JobOpts`], returning its
    /// server-side id. Used by the integration routes (Bazarr, Jellyfin) to feed
    /// the same queue nodes pull from.
    pub fn enqueue(&self, kind: TranscriptionTask, opts: &JobOpts) -> Result<JobId, ServerError> {
        let payload = serde_json::to_string(opts)
            .map_err(|e| ServerError::Internal(format!("encode job opts: {e}")))?;
        let id = self
            .store()
            .enqueue(&NewJob::now(kind.to_string(), payload))?;
        Ok(id)
    }

    /// Enqueue a job and record its [`AudioSource`] so the node can later
    /// `GET /jobs/{id}/audio` to pull the PCM. This is the path the integration
    /// routes (Bazarr, Jellyfin) use: Bazarr relays uploaded PCM
    /// ([`AudioSource::Pcm`]), Jellyfin/local-scan points at a media file the
    /// server extracts on demand ([`AudioSource::File`]).
    pub fn enqueue_with_audio(
        &self,
        kind: TranscriptionTask,
        opts: &JobOpts,
        source: AudioSource,
    ) -> Result<JobId, ServerError> {
        let id = self.enqueue(kind, opts)?;
        self.audio
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(id, source);
        Ok(id)
    }

    /// Materialise a job's audio as raw s16le/mono/16k PCM for the node to fetch.
    ///
    /// A [`AudioSource::Pcm`] source is relayed verbatim; a
    /// [`AudioSource::File`] source is extracted with submate-media (the same
    /// `prepare_audio_for_transcription` the transcription pipeline uses, so the
    /// bytes a node fetches are byte-identical to a local extraction). A job with
    /// no recorded source — or an unknown id — yields `404`.
    ///
    /// The audio lock is only held to clone the (cheap) source descriptor; the
    /// extraction itself, which spawns `ffmpeg`, runs without any lock held.
    async fn audio_for(&self, id: JobId) -> Result<Vec<u8>, ServerError> {
        let source = self
            .audio
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&id)
            .cloned()
            .ok_or_else(|| ServerError::NotFound(format!("no audio for job {id}")))?;

        match source {
            AudioSource::Pcm(bytes) => Ok((*bytes).clone()),
            AudioSource::File { path, language } => {
                match prepare_audio_for_transcription(&path, language.as_deref()).await {
                    PreparedAudio::Pcm(pcm) => Ok(pcm),
                    // A single-track (or unprobeable) file degrades to the path;
                    // the node still wants raw PCM, so extract track 0 directly.
                    PreparedAudio::Path(path) => {
                        submate_media::extract_audio_track_to_memory(&path, 0)
                            .await
                            .map_err(|e| {
                                ServerError::Internal(format!("extract audio for job {id}: {e}"))
                            })
                    }
                }
            }
        }
    }

    /// Register a node, returning the bearer token for its subsequent calls.
    ///
    /// Re-registration is idempotent on the token: a node that reconnects (same
    /// `id`) keeps its existing token while its advertised capabilities are
    /// refreshed, so an in-flight lease keyed by `node_id` survives a re-register.
    fn register(&self, req: NodeRegister) -> NodeRegistered {
        let mut nodes = self.nodes.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let token = nodes
            .get(&req.id).map_or_else(|| format!("tok_{}", req.id), |existing| existing.token.clone());
        nodes.insert(
            req.id,
            NodeInfo {
                token: token.clone(),
                gpu: req.gpu,
                runners: req.runners,
                tasks: req.tasks,
            },
        );
        NodeRegistered { token }
    }

    /// Atomically claim the next eligible job for `node_id`, hydrating it into a
    /// [`WorkResponse::Work`]. The node must have registered first. Returns
    /// `None` when there is nothing to claim (the route renders that as
    /// `204 No Content`).
    ///
    /// If the claimed job's kind is not in the node's advertised `tasks`, the
    /// claim is rolled back (the job returns to `queued`) and `None` is
    /// returned, so a translation-only node never strands a transcribe job. GPU
    /// affinity is advertised via [`NodeInfo::gpu`] for the routing refinement
    /// in later backlog items.
    fn request_work(&self, node_id: &str) -> Result<Option<WorkResponse>, ServerError> {
        let caps = self
            .nodes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(node_id)
            .map(|info| (info.gpu, info.runners, info.tasks.clone()));
        let Some((gpu, runners, tasks)) = caps else {
            return Err(ServerError::BadRequest(format!(
                "node {node_id:?} is not registered"
            )));
        };
        tracing::debug!(node = node_id, gpu, runners, "request-work");

        let store = self.store();
        let Some(job) = store.claim(node_id)? else {
            return Ok(None);
        };

        let kind: TranscriptionTask = job.kind.parse().map_err(|_| {
            ServerError::Internal(format!("job {} has unknown kind {:?}", job.id, job.kind))
        })?;

        if !tasks.contains(&kind) {
            // The node cannot run this kind: release the lease back to `queued`
            // (without consuming an attempt) so a capable node can claim it, and
            // report "no work" to this node.
            store.release(job.id)?;
            return Ok(None);
        }

        let opts: JobOpts = serde_json::from_str(&job.payload)
            .map_err(|e| ServerError::Internal(format!("decode job {} opts: {e}", job.id)))?;

        Ok(Some(WorkResponse::Work {
            job_id: job.id.to_string(),
            kind,
            audio_url: format!("{}/{}/audio", self.audio_base, job.id),
            opts,
        }))
    }

    /// Record an in-flight progress update. The job must exist (unknown id →
    /// `404`); progress itself is advisory, so an update for a job that has
    /// already moved past `running` is accepted and logged rather than erroring.
    fn progress(&self, update: &Progress) -> Result<(), ServerError> {
        let id = parse_job_id(&update.job_id)?;
        let job = self
            .store()
            .get(id)?
            .ok_or_else(|| ServerError::NotFound(format!("job {id} not found")))?;
        tracing::debug!(job = id, pct = update.pct, state = ?job.state, "progress");
        self.fan_out_progress(id, update.clone());
        Ok(())
    }

    /// Deliver a progress update to the job's registered subscriber, if any.
    /// A send failure (the receiver was dropped — the caller gave up) drops the
    /// subscription so later updates short-circuit instead of re-locking.
    fn fan_out_progress(&self, id: JobId, update: Progress) {
        let mut subs = self.progress_subs.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(tx) = subs.get(&id)
            && tx.send(update).is_err() {
                subs.remove(&id);
            }
    }

    /// Mark a job terminal from the node's [`JobResult`] and wake any synchronous
    /// waiter parked on it. A successful outcome completes the job; a failure is
    /// routed through the store's retry/backoff (`fail`), so a transient node
    /// error re-queues the job for another node rather than dropping it.
    fn result(&self, result: JobResult) -> Result<(), ServerError> {
        let id = parse_job_id(&result.job_id)?;
        match &result.outcome {
            JobOutcome::Ok { .. } => {
                self.store().complete(id)?;
            }
            JobOutcome::Err { .. } => {
                self.store().fail(id)?;
            }
        }
        self.wake_waiter(id, result.outcome);
        Ok(())
    }

    /// Extend the lease on every job currently held by `node_id`. A node that
    /// stops heartbeating has its leases expire, and the next
    /// [`reclaim_stale_leases`](JobStore::reclaim_stale_leases) sweep returns its
    /// jobs to `queued` for another node. Returns the number of leases refreshed.
    fn heartbeat(&self, node_id: &str) -> Result<usize, ServerError> {
        Ok(self.store().touch_leases(node_id)?)
    }

    /// Reclaim jobs whose holding node went silent (lease expired). Exposed so a
    /// background sweep can run it periodically; also runs implicitly via the
    /// store on startup. Returns the number of jobs returned to `queued`.
    pub fn reclaim_stale_leases(&self) -> Result<usize, ServerError> {
        Ok(self.store().reclaim_stale_leases()?)
    }

    /// Park until the given job reports a terminal result, returning that
    /// outcome. Used by synchronous callers (e.g. the Bazarr ASR route) that
    /// enqueue a job and block on its subtitle output. Returns `None` if the
    /// waiter channel is dropped before a result arrives.
    pub async fn wait_for_result(&self, id: JobId) -> Option<JobOutcome> {
        let rx = {
            let (tx, rx) = oneshot::channel();
            self.waiters
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .insert(id, tx);
            rx
        };
        rx.await.ok()
    }

    /// Subscribe to a job's in-flight [`Progress`] stream, returning a receiver
    /// that yields each update [`progress`](NodeCoordinator::progress) records,
    /// in arrival order. Used by synchronous callers (e.g. `transcribe --sync`)
    /// to render live progress while parked on
    /// [`wait_for_result`](NodeCoordinator::wait_for_result).
    ///
    /// The channel closes when the job reaches a terminal result (the sender is
    /// dropped in [`result`](NodeCoordinator::result)) or when the coordinator is
    /// dropped, so a consumer can `recv().await` until `None`. A second
    /// subscription for the same job replaces the first.
    pub fn subscribe_progress(&self, id: JobId) -> mpsc::UnboundedReceiver<Progress> {
        let (tx, rx) = mpsc::unbounded_channel();
        self.progress_subs
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(id, tx);
        rx
    }

    fn wake_waiter(&self, id: JobId, outcome: JobOutcome) {
        // Close the progress stream first so a subscriber sees the channel end
        // right as (or before) the terminal outcome arrives.
        self.progress_subs
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&id);
        if let Some(tx) = self
            .waiters
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&id)
        {
            // A dropped receiver (caller gave up) is fine; the result is already
            // durably recorded in the store.
            let _ = tx.send(outcome);
        }
    }
}

impl StatsSource for NodeCoordinator {
    fn stats(&self) -> QueueStats {
        let store = self.store();
        let count = |state| store.count(state).unwrap_or(0).max(0) as u64;
        let nodes = self.nodes.lock().unwrap_or_else(std::sync::PoisonError::into_inner).len() as u64;
        QueueStats {
            pending: count(JobState::Queued),
            running: count(JobState::Running),
            done: count(JobState::Done),
            nodes,
        }
    }
}

fn parse_job_id(raw: &str) -> Result<JobId, ServerError> {
    raw.parse::<JobId>()
        .map_err(|_| ServerError::BadRequest(format!("invalid job id {raw:?}")))
}

/// Configuration for the in-process ("embedded") node that `submate server` runs
/// by default — FileFlows' "Internal Node": a single box processes its own
/// queue with no separate worker process.
///
/// The embedded node is an ordinary [`submate_node::Agent`] pointed at the
/// server's own loopback address, so it travels the exact same pull-based path
/// (`register` → `request-work` → fetch audio → `result`) as a remote node; it
/// just shares the process. Set [`enabled`](EmbeddedNodeSettings::enabled) to
/// `false` for a brain-only deployment that coordinates remote nodes and does no
/// local compute.
///
/// `node_id` distinguishes this node in the registry (and in `GET /queue`'s
/// `nodes` count); `runners` is the local concurrency cap; `gpu`/`tasks` are the
/// advertised capabilities used for job routing.
#[derive(Debug, Clone)]
pub struct EmbeddedNodeSettings {
    /// Run an in-process node alongside the server. `false` ⇒ brain-only.
    pub enabled: bool,
    /// Registry id for the embedded node (its row in `GET /queue`'s `nodes`).
    pub node_id: String,
    /// Whether the embedded node advertises a usable GPU.
    pub gpu: bool,
    /// Local concurrency cap (the dispatcher's runner count). At least 1.
    pub runners: u32,
    /// Job kinds the embedded node will accept.
    pub tasks: Vec<TranscriptionTask>,
}

impl EmbeddedNodeSettings {
    /// Derive the embedded-node defaults from [`ServerSettings`]: enabled, with
    /// the runner count taken from `concurrent_transcriptions` and both job
    /// kinds advertised. A box configured for `concurrent_transcriptions = 0`
    /// still gets a single runner so it can make progress.
    pub fn from_server(server: &ServerSettings) -> Self {
        Self {
            enabled: true,
            node_id: "embedded".to_string(),
            gpu: false,
            runners: server.concurrent_transcriptions.max(1),
            tasks: vec![TranscriptionTask::Transcribe, TranscriptionTask::Translate],
        }
    }
}

impl Default for EmbeddedNodeSettings {
    fn default() -> Self {
        Self::from_server(&ServerSettings::default())
    }
}

/// Spawn the in-process node and return its run-loop handle, or `None` when the
/// embedded node is disabled (brain-only deployment).
///
/// `base_url` is the server's own reachable address (e.g.
/// `http://127.0.0.1:9000`); the agent talks to it over loopback exactly as a
/// remote node would, so there is no special in-process bypass to keep in sync
/// with the real coordination path. `processor` is the local compute seam — in
/// production this is [`submate_node::whisper_processor`] (behind the node's
/// `model` feature); the falsifier injects a mock that returns a canned subtitle.
///
/// `translation` is the optional translation post-step: when `Some`, a job
/// carrying a `target_language` has its assembled subtitle translated before the
/// result is reported; `None` makes the node transcription-only.
///
/// The returned [`JoinHandle`] runs the agent's pull-loop for the lifetime of
/// the server; dropping it (or shutting the runtime down) stops the node. The
/// agent reconnects with backoff if the server is briefly unreachable, so a
/// short race between binding the listener and starting the node is harmless.
pub fn spawn_embedded_node<P>(
    base_url: impl Into<String>,
    settings: &EmbeddedNodeSettings,
    processor: P,
    translation: Option<submate_node::TranslationStep>,
) -> Option<JoinHandle<Result<(), AgentError>>>
where
    P: JobProcessor + 'static,
{
    if !settings.enabled {
        return None;
    }
    let register = NodeRegister {
        id: settings.node_id.clone(),
        gpu: settings.gpu,
        runners: settings.runners,
        tasks: settings.tasks.clone(),
    };
    let dispatcher = Dispatcher::new(settings.runners.max(1) as usize);
    let mut agent = Agent::new(base_url, register, dispatcher, processor);
    if let Some(step) = translation {
        agent = agent.with_translation(step);
    }
    Some(tokio::spawn(async move { agent.run().await }))
}

/// Build the full server [`Router`], mounting the always-on ops routes plus any
/// feature-enabled integration routers.
pub fn app(state: AppState) -> Router {
    let router = ops_router().merge(node_router());

    #[cfg(feature = "bazarr")]
    let router = router.merge(bazarr_router());

    #[cfg(feature = "jellyfin")]
    let router = router.merge(jellyfin_router());

    router.with_state(state)
}

/// The ops routes, mirroring `submate/server/handlers/core/router.py`.
fn ops_router() -> Router<AppState> {
    Router::new()
        .route("/", get(root))
        .route("/status", get(status))
        .route("/queue", get(queue))
}

/// The node-coordination routes (FileFlows/Unmanic topology, see
/// `rust/docs/architecture.md`): node lifecycle (`register`, `request-work`,
/// `heartbeat`) and job reporting (`progress`, `result`). All operate on the
/// merged [`NodeCoordinator`]; without one wired in they return `503`.
fn node_router() -> Router<AppState> {
    Router::new()
        .route("/nodes/register", post(nodes_register))
        .route("/nodes/:id/request-work", post(nodes_request_work))
        .route("/nodes/:id/heartbeat", post(nodes_heartbeat))
        .route("/jobs/:id/audio", get(jobs_audio))
        .route("/jobs/:id/progress", post(jobs_progress))
        .route("/jobs/:id/result", post(jobs_result))
}

/// `POST /nodes/register` — announce capabilities, receive a coordination token.
async fn nodes_register(
    State(state): State<AppState>,
    Json(req): Json<NodeRegister>,
) -> std::result::Result<Json<NodeRegistered>, ServerError> {
    Ok(Json(state.coordinator()?.register(req)))
}

/// `POST /nodes/{id}/request-work` — atomically claim the next eligible job, or
/// `204 No Content` when the (long-)poll finds nothing claimable.
async fn nodes_request_work(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> std::result::Result<Response, ServerError> {
    match state.coordinator()?.request_work(&id)? {
        Some(work) => Ok(Json(work).into_response()),
        None => Ok(StatusCode::NO_CONTENT.into_response()),
    }
}

/// `POST /nodes/{id}/heartbeat` — extend the node's leases; reports how many
/// jobs were refreshed.
async fn nodes_heartbeat(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> std::result::Result<Json<serde_json::Value>, ServerError> {
    let refreshed = state.coordinator()?.heartbeat(&id)?;
    Ok(Json(json!({ "leases": refreshed })))
}

/// `GET /jobs/{id}/audio` — stream the job's extracted PCM to the node.
///
/// This is the audio-transfer half of the pull topology: the `request-work`
/// response advertises `audio_url = {audio_base}/{id}/audio`, and the node
/// `GET`s here to pull the s16le/mono/16k PCM rather than receiving it inlined
/// in JSON. The body is served as `application/octet-stream`; an unknown job (or
/// one with no recorded audio source) returns `404`.
async fn jobs_audio(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> std::result::Result<Response, ServerError> {
    let job_id = parse_job_id(&id)?;
    let pcm = state.coordinator()?.audio_for(job_id).await?;
    Ok((
        [(header::CONTENT_TYPE, "application/octet-stream")],
        Body::from(pcm),
    )
        .into_response())
}

/// `POST /jobs/{id}/progress` — record an in-flight progress update.
async fn jobs_progress(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(mut update): Json<Progress>,
) -> std::result::Result<StatusCode, ServerError> {
    // The path id is authoritative; ignore a mismatched body job_id.
    update.job_id = id;
    state.coordinator()?.progress(&update)?;
    Ok(StatusCode::NO_CONTENT)
}

/// `POST /jobs/{id}/result` — mark the job terminal and wake any waiter.
async fn jobs_result(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(mut result): Json<JobResult>,
) -> std::result::Result<StatusCode, ServerError> {
    result.job_id = id;
    state.coordinator()?.result(result)?;
    Ok(StatusCode::NO_CONTENT)
}

/// `GET /` — server-info object (matches Python's `root`).
async fn root() -> Json<serde_json::Value> {
    Json(json!({
        "name": "Submate Server",
        "version": VERSION,
        "docs": "/docs",
        "endpoints": {
            "bazarr_asr": "/bazarr/asr",
            "bazarr_detect_language": "/bazarr/detect-language",
            "jellyfin": "/webhooks/jellyfin",
            "status": "/status",
            "queue": "/queue",
        },
    }))
}

/// `GET /status` — health + version + queue snapshot (matches Python's `status`).
async fn status(State(state): State<AppState>) -> Json<serde_json::Value> {
    Json(json!({
        "status": "ok",
        "version": VERSION,
        "queue": state.stats.stats(),
    }))
}

/// `GET /queue` — node-topology queue statistics (matches Python's `queue_status`).
async fn queue(State(state): State<AppState>) -> Json<QueueStats> {
    Json(state.stats.stats())
}

/// The Bazarr routes (`POST /bazarr/asr`, `POST /bazarr/detect-language`),
/// mirroring `submate/server/handlers/bazarr/router.py`. They run a **direct**
/// transcription via the [`BazarrTranscriber`] seam — Bazarr's Whisper provider
/// is synchronous, so the durable queue is deliberately off this path.
#[cfg(feature = "bazarr")]
fn bazarr_router() -> Router<AppState> {
    Router::new()
        .route("/bazarr/asr", post(bazarr_asr))
        .route("/bazarr/detect-language", post(bazarr_detect_language))
}

/// `Source` response header the Python `/bazarr/asr` handler sets.
#[cfg(feature = "bazarr")]
const BAZARR_SOURCE: &str = "Transcribed using stable-ts from Submate";

/// `POST /bazarr/asr` query params (wire-exact with the Python `Query(...)`).
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
    /// Whisper decode hint; source is auto-detected (mirrors the Python handler).
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

/// Jellyfin webhook router.
///
/// Mounts `POST /webhooks/jellyfin`, mirroring
/// `submate/server/handlers/jellyfin/router.py` (router prefix `/webhooks`,
/// path `/jellyfin`). This establishes the contract-correct route path and
/// payload shape; the enqueue/skip pipeline behind it is filled in by
/// `backlog/port-server-jellyfin-webhook.md`.
#[cfg(feature = "jellyfin")]
fn jellyfin_router() -> Router<AppState> {
    Router::new().route("/webhooks/jellyfin", post(jellyfin_webhook))
}

/// `POST /webhooks/jellyfin` — accept a Jellyfin webhook notification.
///
/// Validates the `User-Agent` (must come from a Jellyfin server) and parses the
/// PascalCase [`JellyfinWebhookPayload`], then mirrors the response *shape* of
/// Python's `handle_jellyfin_webhook`:
///
/// - **skipped** — the event is not configured for processing (the
///   `process_on_add` / `process_on_play` gate): `{"status": "skipped",
///   "message": "Event {notification_type} not configured"}`.
/// - **queued** — enqueue succeeds: `{"status": "queued", "task_id": <ItemId>,
///   "file_path": <mapped_path>}`, where `task_id` is the *ItemId* (not an
///   internal job id).
/// - **error** — processing raises: `{"status": "error", "message": <str>}`.
///
/// Only the skipped path (deterministic, no node/queue) is wired here; the
/// file-path resolution and enqueue behind the should-process branch are filled
/// in by `backlog/port-server-jellyfin-webhook.md`. Until then the
/// should-process branch reports the `error` shape, which is a valid Python
/// response value.
#[cfg(feature = "jellyfin")]
async fn jellyfin_webhook(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Json(payload): Json<submate_jellyfin::JellyfinWebhookPayload>,
) -> std::result::Result<Json<serde_json::Value>, ServerError> {
    let user_agent = headers
        .get(axum::http::header::USER_AGENT)
        .and_then(|v| v.to_str().ok());
    if !user_agent.is_some_and(|ua| ua.contains("Jellyfin-Server")) {
        return Err(ServerError::BadRequest(
            "Invalid request - not from Jellyfin server".to_string(),
        ));
    }

    let should_process = (payload.is_item_added() && state.server.process_on_add)
        || (payload.is_playback_start() && state.server.process_on_play);

    if !should_process {
        return Ok(Json(json!({
            "status": "skipped",
            "message": format!("Event {} not configured", payload.notification_type),
        })));
    }

    // Enqueue pipeline (file-path resolution + job enqueue) is wired by
    // backlog/port-server-jellyfin-webhook.md. Report the Python `error` shape
    // until then rather than the foreign "accepted" shape.
    Ok(Json(json!({
        "status": "error",
        "message": "Jellyfin enqueue pipeline not yet wired",
    })))
}

#[cfg(test)]
mod tests {
    use super::*;

    use axum::body::Body;
    use axum::http::Request;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    async fn get_json(app: Router, uri: &str) -> (StatusCode, serde_json::Value) {
        let res = app
            .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = res.status();
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        (status, body)
    }

    // The literal `GET /` and `GET /status` bodies (server name, version, docs,
    // and all five `endpoints` paths) are pinned against the Python-captured
    // golden in `tests/parity.rs` (`core_router::root` / `core_router::status`),
    // not hand-encoded here, so the Rust handlers cannot silently drift from the
    // `core/router.py` SPEC. This inline test keeps only the *structural* guard
    // that is local to the Rust shape: `GET /status` has exactly the three
    // top-level keys and its `queue` value is an object.
    #[tokio::test]
    async fn ops_routes_status_top_level_shape() {
        let (status, body) = get_json(app(AppState::default()), "/status").await;
        assert_eq!(status, StatusCode::OK);
        let obj = body.as_object().unwrap();
        assert_eq!(obj.len(), 3);
        assert!(obj.contains_key("status"));
        assert!(obj.contains_key("version"));
        assert!(body["queue"].is_object());
    }

    #[tokio::test]
    async fn ops_routes_queue_returns_zeroed_node_topology_stats() {
        let (status, body) = get_json(app(AppState::default()), "/queue").await;
        assert_eq!(status, StatusCode::OK);
        // Node-topology shape, zeroed on an empty queue.
        assert_eq!(body["pending"], 0);
        assert_eq!(body["running"], 0);
        assert_eq!(body["done"], 0);
        assert_eq!(body["nodes"], 0);
    }

    #[tokio::test]
    async fn ops_routes_queue_reflects_live_stats() {
        struct Fixed(QueueStats);
        impl StatsSource for Fixed {
            fn stats(&self) -> QueueStats {
                self.0
            }
        }
        let stats = QueueStats {
            pending: 3,
            running: 1,
            done: 7,
            nodes: 2,
        };
        let (status, body) = get_json(app(AppState::new(Fixed(stats))), "/queue").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["pending"], 3);
        assert_eq!(body["running"], 1);
        assert_eq!(body["done"], 7);
        assert_eq!(body["nodes"], 2);
    }

    #[tokio::test]
    async fn unknown_route_is_not_found() {
        let res = app(AppState::default())
            .oneshot(
                Request::builder()
                    .uri("/does-not-exist")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }

    #[cfg(feature = "jellyfin")]
    #[tokio::test]
    async fn jellyfin_webhook_route_mounted_at_webhooks_jellyfin() {
        let body = serde_json::to_vec(&json!({
            "NotificationType": "ItemAdded",
            "ItemId": "abc",
            "ItemType": "Movie",
        }))
        .unwrap();
        let res = app(AppState::default())
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/webhooks/jellyfin")
                    .header("content-type", "application/json")
                    .header("user-agent", "Jellyfin-Server/10.9.0")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[cfg(feature = "jellyfin")]
    #[tokio::test]
    async fn jellyfin_webhook_response_shape() {
        // Deterministic, node-free falsifier: a valid Jellyfin `ItemAdded`
        // webhook against a server with `process_on_add = false` must return
        // exactly the Python "skipped" shape — `{status, message}` and nothing
        // else.
        let server = ServerSettings {
            process_on_add: false,
            ..ServerSettings::default()
        };
        let state = AppState::default().with_server_settings(server);

        let body = serde_json::to_vec(&json!({
            "NotificationType": "ItemAdded",
            "ItemId": "abc",
            "ItemType": "Movie",
        }))
        .unwrap();
        let res = app(state)
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/webhooks/jellyfin")
                    .header("content-type", "application/json")
                    .header("user-agent", "Jellyfin-Server/10.9.0")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();

        let obj = json.as_object().unwrap();
        assert_eq!(obj.len(), 2, "skipped body has exactly two keys");
        assert_eq!(json["status"], "skipped");
        assert_eq!(json["message"], "Event ItemAdded not configured");
        // The foreign skeleton keys must be gone.
        assert!(obj.get("notification_type").is_none());
        assert!(obj.get("item_id").is_none());
    }

    #[cfg(feature = "jellyfin")]
    #[tokio::test]
    async fn jellyfin_webhook_rejects_non_jellyfin_user_agent() {
        let body = serde_json::to_vec(&json!({
            "NotificationType": "ItemAdded",
            "ItemId": "abc",
        }))
        .unwrap();
        let res = app(AppState::default())
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/webhooks/jellyfin")
                    .header("content-type", "application/json")
                    .header("user-agent", "curl/8.0")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
        // Python emits FastAPI's `HTTPException` envelope `{"detail": ...}`; the
        // error body must match that shape (not the `error` key).
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            body["detail"], "Invalid request - not from Jellyfin server",
            "FastAPI error bodies use the `detail` key, not `error`"
        );
        assert!(body.get("error").is_none(), "must not use the `error` key");
    }

    #[cfg(feature = "jellyfin")]
    #[tokio::test]
    async fn legacy_webhook_path_is_not_mounted() {
        // The pre-correction path (`/jellyfin` + `/webhook`) must not resolve.
        let legacy = format!("/{}/{}", "jellyfin", "webhook");
        let res = app(AppState::default())
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(&legacy)
                    .header("content-type", "application/json")
                    .body(Body::from("{}"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn server_error_renders_json_envelope_with_status() {
        use http_body_util::BodyExt;

        let res = ServerError::NotFound("nope".into()).into_response();
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
        let res = ServerError::BadRequest("bad".into()).into_response();
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
        let res = ServerError::Unavailable("nope".into()).into_response();
        assert_eq!(res.status(), StatusCode::SERVICE_UNAVAILABLE);

        // The 500 envelope matches FastAPI's global handler: `{"detail": ...}`.
        let res = ServerError::Internal("boom".into()).into_response();
        assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(body["detail"], "boom");
        assert!(body.get("error").is_none(), "must not use the `error` key");
    }

    // ---- node-coordination API ---------------------------------------------

    use std::sync::atomic::{AtomicI64, Ordering};
    use submate_queue::{Clock, JobState, JobStore, StoreConfig};
    use submate_types::{Device, TranscriptionTask, WhisperModel};

    /// A controllable clock so the lease-reclaim falsifier advances time
    /// deterministically instead of sleeping.
    #[derive(Clone, Default)]
    struct TestClock(Arc<AtomicI64>);
    impl TestClock {
        fn new(ms: i64) -> Self {
            Self(Arc::new(AtomicI64::new(ms)))
        }
        fn set(&self, ms: i64) {
            self.0.store(ms, Ordering::SeqCst);
        }
    }
    impl Clock for TestClock {
        fn now_ms(&self) -> i64 {
            self.0.load(Ordering::SeqCst)
        }
    }

    fn transcribe_opts() -> JobOpts {
        JobOpts {
            model: WhisperModel::Medium,
            device: Device::Cpu,
            source_language: None,
            target_language: None,
            translation_backend: None,
            output_format: submate_proto::OutputFormat::default(),
        }
    }

    /// POST `body` to `uri`, returning the status and (parsed if any) JSON body.
    async fn post_json(
        app: Router,
        uri: &str,
        body: serde_json::Value,
    ) -> (StatusCode, Option<serde_json::Value>) {
        let res = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(uri)
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = res.status();
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        let json = (!bytes.is_empty()).then(|| serde_json::from_slice(&bytes).unwrap());
        (status, json)
    }

    /// Falsifier: a node registers, a job is enqueued, `request-work` hands it to
    /// the node, posting a result marks it done, a heartbeat extends the lease,
    /// and an un-heartbeated node's job is reclaimed once the lease expires.
    #[tokio::test]
    async fn node_api_roundtrip() {
        let clock = TestClock::new(1_000);
        let store = JobStore::in_memory_with(
            StoreConfig {
                lease_ms: 5_000,
                ..StoreConfig::default()
            },
            Box::new(clock.clone()),
        )
        .unwrap();
        let coord = Arc::new(NodeCoordinator::new(store));

        // Enqueue a transcribe job (the server side does this for Bazarr/Jellyfin).
        let job_id = coord
            .enqueue(TranscriptionTask::Transcribe, &transcribe_opts())
            .unwrap();

        // Register the node -> token.
        let (status, body) = post_json(
            app(AppState::with_coordinator(coord.clone())),
            "/nodes/register",
            serde_json::to_value(NodeRegister {
                id: "node-1".into(),
                gpu: true,
                runners: 2,
                tasks: vec![TranscriptionTask::Transcribe],
            })
            .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let registered: NodeRegistered = serde_json::from_value(body.unwrap()).unwrap();
        assert!(!registered.token.is_empty());

        // request-work returns the enqueued job.
        let (status, body) = post_json(
            app(AppState::with_coordinator(coord.clone())),
            "/nodes/node-1/request-work",
            json!({ "node_id": "node-1" }),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let work: WorkResponse = serde_json::from_value(body.unwrap()).unwrap();
        match work {
            WorkResponse::Work {
                job_id: jid,
                kind,
                audio_url,
                ..
            } => {
                assert_eq!(jid, job_id.to_string());
                assert_eq!(kind, TranscriptionTask::Transcribe);
                assert_eq!(audio_url, format!("/jobs/{job_id}/audio"));
            }
            WorkResponse::NoWork => panic!("expected work"),
        }
        // Now claimed/running.
        assert_eq!(
            coord.stats(),
            QueueStats {
                pending: 0,
                running: 1,
                done: 0,
                nodes: 1
            }
        );

        // A second poll has nothing to claim -> 204.
        let (status, body) = post_json(
            app(AppState::with_coordinator(coord.clone())),
            "/nodes/node-1/request-work",
            json!({ "node_id": "node-1" }),
        )
        .await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        assert!(body.is_none());

        // Heartbeat at t=4000 (within the 5s lease) extends the lease.
        clock.set(4_000);
        let (status, body) = post_json(
            app(AppState::with_coordinator(coord.clone())),
            "/nodes/node-1/heartbeat",
            json!({ "node_id": "node-1" }),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body.unwrap()["leases"], 1);

        // At t=8000 the lease would be stale relative to the *original* claim
        // (1000 + 5000 = 6000), but the heartbeat re-stamped it to 4000, so
        // 4000 + 5000 = 9000 > 8000 -> still held, not reclaimed.
        clock.set(8_000);
        assert_eq!(coord.reclaim_stale_leases().unwrap(), 0);
        assert_eq!(coord.stats().running, 1);

        // Progress update is accepted (204).
        let (status, _) = post_json(
            app(AppState::with_coordinator(coord.clone())),
            &format!("/jobs/{job_id}/progress"),
            json!({ "job_id": job_id.to_string(), "pct": 0.5 }),
        )
        .await;
        assert_eq!(status, StatusCode::NO_CONTENT);

        // Posting a successful result marks the job done and wakes a waiter.
        let waiter = {
            let coord = coord.clone();
            tokio::spawn(async move { coord.wait_for_result(job_id).await })
        };
        // Give the waiter a moment to register before the result arrives.
        tokio::task::yield_now().await;

        let (status, _) = post_json(
            app(AppState::with_coordinator(coord.clone())),
            &format!("/jobs/{job_id}/result"),
            json!({ "job_id": job_id.to_string(), "ok": true, "output": "subs" }),
        )
        .await;
        assert_eq!(status, StatusCode::NO_CONTENT);

        let outcome = waiter.await.unwrap();
        assert_eq!(
            outcome,
            Some(JobOutcome::Ok {
                output: "subs".into()
            })
        );
        assert_eq!(
            coord.stats(),
            QueueStats {
                pending: 0,
                running: 0,
                done: 1,
                nodes: 1
            }
        );
    }

    /// A non-heartbeating node's job is reclaimed once its lease expires, and is
    /// then handed to a live node on the next `request-work`.
    #[tokio::test]
    async fn unheartbeated_node_job_is_reclaimed() {
        let clock = TestClock::new(0);
        let store = JobStore::in_memory_with(
            StoreConfig {
                lease_ms: 5_000,
                ..StoreConfig::default()
            },
            Box::new(clock.clone()),
        )
        .unwrap();
        let coord = Arc::new(NodeCoordinator::new(store));
        coord
            .enqueue(TranscriptionTask::Transcribe, &transcribe_opts())
            .unwrap();
        coord.register(NodeRegister {
            id: "dead".into(),
            gpu: false,
            runners: 1,
            tasks: vec![TranscriptionTask::Transcribe],
        });

        // Dead node claims, then never heartbeats.
        assert!(matches!(
            coord.request_work("dead").unwrap(),
            Some(WorkResponse::Work { .. })
        ));
        assert_eq!(coord.stats().running, 1);

        // Past the lease window -> reclaimed back to queued.
        clock.set(6_000);
        assert_eq!(coord.reclaim_stale_leases().unwrap(), 1);
        assert_eq!(coord.stats().pending, 1);
        assert_eq!(coord.stats().running, 0);

        // A live node now picks it up.
        coord.register(NodeRegister {
            id: "live".into(),
            gpu: false,
            runners: 1,
            tasks: vec![TranscriptionTask::Transcribe],
        });
        assert!(matches!(
            coord.request_work("live").unwrap(),
            Some(WorkResponse::Work { .. })
        ));
    }

    /// A subscriber registered via `subscribe_progress` receives every
    /// in-flight `Progress` update, in order, and the stream closes when the job
    /// reaches a terminal result.
    #[tokio::test]
    async fn coordinator_progress_subscription() {
        let store = JobStore::open_in_memory().unwrap();
        let coord = Arc::new(NodeCoordinator::new(store));
        let job_id = coord
            .enqueue(TranscriptionTask::Transcribe, &transcribe_opts())
            .unwrap();
        // Claim the job so it is `running` (progress requires the job to exist).
        coord.register(NodeRegister {
            id: "node".into(),
            gpu: false,
            runners: 1,
            tasks: vec![TranscriptionTask::Transcribe],
        });
        assert!(coord.request_work("node").unwrap().is_some());

        let mut rx = coord.subscribe_progress(job_id);

        // Post a 0 -> 100 sequence; each must arrive at the subscriber in order.
        let pcts = [0.0f32, 0.25, 0.5, 0.75, 1.0];
        for pct in pcts {
            coord
                .progress(&Progress {
                    job_id: job_id.to_string(),
                    pct,
                })
                .unwrap();
        }

        for expected in pcts {
            let got = rx.recv().await.expect("update delivered");
            assert_eq!(got.pct, expected);
            assert_eq!(got.job_id, job_id.to_string());
        }

        // Terminal result closes the stream: the receiver drains to `None`.
        coord
            .result(JobResult {
                job_id: job_id.to_string(),
                outcome: JobOutcome::Ok {
                    output: "subs".into(),
                },
            })
            .unwrap();
        assert!(rx.recv().await.is_none());
    }

    /// A node whose advertised tasks do not cover the claimed job releases it
    /// (no attempt consumed) so a capable node can take it.
    #[tokio::test]
    async fn capability_mismatch_releases_job() {
        let store = JobStore::open_in_memory().unwrap();
        let coord = Arc::new(NodeCoordinator::new(store));
        coord
            .enqueue(TranscriptionTask::Transcribe, &transcribe_opts())
            .unwrap();

        // Translate-only node: claim is rolled back, sees no work.
        coord.register(NodeRegister {
            id: "translate-only".into(),
            gpu: false,
            runners: 1,
            tasks: vec![TranscriptionTask::Translate],
        });
        assert!(coord.request_work("translate-only").unwrap().is_none());
        // Job is back to queued with its single attempt intact.
        assert_eq!(coord.stats().pending, 1);

        // A transcribe-capable node gets it.
        coord.register(NodeRegister {
            id: "full".into(),
            gpu: false,
            runners: 1,
            tasks: vec![TranscriptionTask::Transcribe, TranscriptionTask::Translate],
        });
        assert!(matches!(
            coord.request_work("full").unwrap(),
            Some(WorkResponse::Work { .. })
        ));
    }

    /// An unregistered node cannot request work.
    #[tokio::test]
    async fn request_work_requires_registration() {
        let store = JobStore::open_in_memory().unwrap();
        let coord = Arc::new(NodeCoordinator::new(store));
        let err = coord.request_work("ghost").unwrap_err();
        assert!(matches!(err, ServerError::BadRequest(_)));
    }

    /// Without a coordinator wired in, node routes report `503` rather than 404.
    #[tokio::test]
    async fn node_routes_unavailable_without_coordinator() {
        let (status, _) = post_json(
            app(AppState::default()),
            "/nodes/register",
            serde_json::to_value(NodeRegister {
                id: "n".into(),
                gpu: false,
                runners: 1,
                tasks: vec![TranscriptionTask::Transcribe],
            })
            .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    }

    // ---- audio transfer -----------------------------------------------------

    /// GET `uri`, returning the status and the raw response body bytes.
    async fn get_bytes(app: Router, uri: &str) -> (StatusCode, Vec<u8>) {
        let res = app
            .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = res.status();
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        (status, bytes.to_vec())
    }

    /// Falsifier `audio_transfer` (relay): a job enqueued with already-decoded
    /// PCM (the Bazarr path) is served verbatim by `GET /jobs/{id}/audio`, and
    /// the fetched bytes' sha256 matches the source PCM. No ffmpeg needed.
    #[tokio::test]
    async fn audio_transfer_relays_uploaded_pcm() {
        use sha2::{Digest, Sha256};

        let store = JobStore::open_in_memory().unwrap();
        let coord = Arc::new(NodeCoordinator::new(store));

        // 320 bytes of deterministic "PCM" (16-bit samples) standing in for the
        // Bazarr-uploaded audio; the route relays bytes opaquely.
        let pcm: Vec<u8> = (0..320u16).flat_map(u16::to_le_bytes).collect();
        let expected = hex::encode(Sha256::digest(&pcm));

        let job_id = coord
            .enqueue_with_audio(
                TranscriptionTask::Transcribe,
                &transcribe_opts(),
                AudioSource::Pcm(Arc::new(pcm.clone())),
            )
            .unwrap();

        let (status, bytes) = get_bytes(
            app(AppState::with_coordinator(coord.clone())),
            &format!("/jobs/{job_id}/audio"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(bytes, pcm, "relayed PCM must be byte-identical");
        assert_eq!(
            hex::encode(Sha256::digest(&bytes)),
            expected,
            "fetched audio sha256 must match the source PCM",
        );
    }

    /// Falsifier `audio_transfer` (extraction): a job whose audio source is a
    /// media file on the server is extracted on demand, and the bytes the node
    /// fetches over `GET /jobs/{id}/audio` are byte-identical (sha256) to a
    /// direct submate-media extraction of the same track. Skipped as a no-op
    /// when `ffmpeg`/`ffprobe` are not on `PATH`.
    #[tokio::test]
    async fn audio_transfer_extracts_file_matching_source() {
        use sha2::{Digest, Sha256};

        fn binary_on_path(name: &str) -> bool {
            std::process::Command::new(name)
                .arg("-version")
                .output()
                .is_ok_and(|o| o.status.success())
        }
        if !binary_on_path("ffmpeg") || !binary_on_path("ffprobe") {
            eprintln!("skipping audio_transfer extraction: ffmpeg/ffprobe not on PATH");
            return;
        }

        // Synthesize a single-track media file (1s silence, 16k mono). Written
        // to a temp path, not a fixture.
        let path =
            std::env::temp_dir().join(format!("submate-server-audio-{}.mka", std::process::id()));
        let r#gen = std::process::Command::new("ffmpeg")
            .args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                "anullsrc=r=16000:cl=mono",
                "-t",
                "1",
                "-c:a",
                "aac",
            ])
            .arg(&path)
            .output()
            .expect("ffmpeg runs");
        assert!(
            r#gen.status.success(),
            "ffmpeg failed: {}",
            String::from_utf8_lossy(&r#gen.stderr)
        );

        // Golden: a direct extraction of the file's only audio track.
        let golden = submate_media::extract_audio_track_to_memory(&path, 0)
            .await
            .expect("direct extraction succeeds");
        let expected = hex::encode(Sha256::digest(&golden));

        let store = JobStore::open_in_memory().unwrap();
        let coord = Arc::new(NodeCoordinator::new(store));
        let job_id = coord
            .enqueue_with_audio(
                TranscriptionTask::Transcribe,
                &transcribe_opts(),
                AudioSource::File {
                    path: path.clone(),
                    language: None,
                },
            )
            .unwrap();

        let (status, bytes) = get_bytes(
            app(AppState::with_coordinator(coord.clone())),
            &format!("/jobs/{job_id}/audio"),
        )
        .await;
        let _ = std::fs::remove_file(&path);

        assert_eq!(status, StatusCode::OK);
        assert!(!bytes.is_empty(), "extracted PCM must be non-empty");
        assert_eq!(
            hex::encode(Sha256::digest(&bytes)),
            expected,
            "fetched audio sha256 must match the source extraction",
        );
    }

    /// A job with no recorded audio source, and a wholly unknown job id, both
    /// return `404` from the audio route.
    #[tokio::test]
    async fn audio_transfer_missing_source_is_not_found() {
        let store = JobStore::open_in_memory().unwrap();
        let coord = Arc::new(NodeCoordinator::new(store));

        // Enqueued via the plain path: no audio source recorded.
        let job_id = coord
            .enqueue(TranscriptionTask::Transcribe, &transcribe_opts())
            .unwrap();
        let (status, _) = get_bytes(
            app(AppState::with_coordinator(coord.clone())),
            &format!("/jobs/{job_id}/audio"),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);

        // A job id that was never enqueued.
        let (status, _) = get_bytes(
            app(AppState::with_coordinator(coord.clone())),
            "/jobs/999999/audio",
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    /// Without a coordinator wired in, the audio route reports `503`, like the
    /// other node routes.
    #[tokio::test]
    async fn audio_route_unavailable_without_coordinator() {
        let (status, _) = get_bytes(app(AppState::default()), "/jobs/1/audio").await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    }

    /// A failed result re-queues the job (retry/backoff) and still wakes the
    /// waiter with the failure.
    #[tokio::test]
    async fn failed_result_requeues_and_wakes_waiter() {
        let store = JobStore::open_in_memory().unwrap();
        let coord = Arc::new(NodeCoordinator::new(store));
        let job_id = coord
            .enqueue(TranscriptionTask::Transcribe, &transcribe_opts())
            .unwrap();
        coord.register(NodeRegister {
            id: "n".into(),
            gpu: false,
            runners: 1,
            tasks: vec![TranscriptionTask::Transcribe],
        });
        let _ = coord.request_work("n").unwrap().unwrap();

        let waiter = {
            let coord = coord.clone();
            tokio::spawn(async move { coord.wait_for_result(job_id).await })
        };
        tokio::task::yield_now().await;

        coord
            .result(JobResult {
                job_id: job_id.to_string(),
                outcome: JobOutcome::Err {
                    error: "boom".into(),
                },
            })
            .unwrap();

        let outcome = waiter.await.unwrap();
        assert_eq!(
            outcome,
            Some(JobOutcome::Err {
                error: "boom".into()
            })
        );
        // Single-attempt job: a failure makes it terminally failed, not done.
        assert_eq!(coord.stats().done, 0);
        let store = coord.store();
        assert_eq!(store.count(JobState::Failed).unwrap(), 1);
    }

    // ---- embedded in-process node ------------------------------------------

    /// Falsifier `embedded_node_drains`: a server with the embedded node enabled
    /// processes an enqueued job end-to-end and marks it done — no separate
    /// worker process.
    ///
    /// We bind the real router on an ephemeral loopback port, spawn the
    /// in-process node ([`spawn_embedded_node`]) pointed at that address with a
    /// mock transcription processor, enqueue a transcribe job (with relayed PCM
    /// so the node's audio fetch succeeds), then park on the coordinator's
    /// [`wait_for_result`](NodeCoordinator::wait_for_result). The node travels
    /// the full pull path over loopback — register → request-work → GET audio →
    /// POST result — and the result wakes the waiter with the mock's subtitle.
    /// Finally the job is `done` in the store.
    #[tokio::test]
    async fn embedded_node_drains() {
        let store = JobStore::open_in_memory().unwrap();
        let coord = Arc::new(NodeCoordinator::new(store));

        // Enqueue a job whose audio the node can fetch (relayed PCM — no ffmpeg).
        let pcm = vec![7u8, 8, 9, 10];
        let job_id = coord
            .enqueue_with_audio(
                TranscriptionTask::Transcribe,
                &transcribe_opts(),
                AudioSource::Pcm(Arc::new(pcm.clone())),
            )
            .unwrap();

        // Serve the real router on an ephemeral loopback port.
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .expect("bind ephemeral port");
        let addr = listener.local_addr().unwrap();
        let server_app = app(AppState::with_coordinator(coord.clone()));
        let server = tokio::spawn(async move {
            axum::serve(listener, server_app).await.unwrap();
        });

        // Mock transcription: assert the node fetched the enqueued PCM, then
        // return a canned subtitle. No model loaded.
        let seen_pcm = Arc::new(Mutex::new(Vec::<u8>::new()));
        let processor = {
            let seen_pcm = Arc::clone(&seen_pcm);
            move |_opts: &JobOpts, got: Vec<u8>| {
                let seen_pcm = Arc::clone(&seen_pcm);
                async move {
                    *seen_pcm.lock().unwrap() = got;
                    Ok::<_, String>("1\n00:00:00,000 --> 00:00:01,000\nhello\n".to_string())
                }
            }
        };

        let settings = EmbeddedNodeSettings {
            enabled: true,
            node_id: "embedded".into(),
            gpu: false,
            runners: 1,
            tasks: vec![TranscriptionTask::Transcribe],
        };
        let base_url = format!("http://{addr}");
        let node = spawn_embedded_node(&base_url, &settings, processor, None)
            .expect("embedded node enabled");

        // Park on the result; the embedded node drains the queue and posts it.
        let outcome = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            coord.wait_for_result(job_id),
        )
        .await
        .expect("embedded node did not drain the job in time");

        assert!(
            matches!(outcome, Some(JobOutcome::Ok { ref output }) if output.contains("hello")),
            "embedded node posted unexpected outcome: {outcome:?}",
        );
        // The node fetched exactly the enqueued PCM over loopback.
        assert_eq!(*seen_pcm.lock().unwrap(), pcm);
        // The job is durably done in the store.
        assert_eq!(coord.stats().done, 1);

        node.abort();
        server.abort();
    }

    /// A brain-only deployment (embedded node disabled) spawns no node, so an
    /// enqueued job stays `pending` with no local compute draining it.
    #[tokio::test]
    async fn embedded_node_disabled_is_brain_only() {
        let store = JobStore::open_in_memory().unwrap();
        let coord = Arc::new(NodeCoordinator::new(store));
        coord
            .enqueue(TranscriptionTask::Transcribe, &transcribe_opts())
            .unwrap();

        let settings = EmbeddedNodeSettings {
            enabled: false,
            ..EmbeddedNodeSettings::default()
        };
        let processor = |_opts: &JobOpts, _pcm: Vec<u8>| async { Ok::<_, String>(String::new()) };
        let handle = spawn_embedded_node("http://127.0.0.1:1", &settings, processor, None);
        assert!(handle.is_none(), "brain-only server must not spawn a node");

        // Nothing drains the job: it stays queued, with no registered nodes.
        assert_eq!(coord.stats().pending, 1);
        assert_eq!(coord.stats().nodes, 0);
    }
}

/// Bazarr Whisper-provider contract tests — drive `app()` with a fake
/// [`BazarrTranscriber`] (no model) and pin the behaviors `whisperai.py`
/// depends on: SRT-in-body + `Source` header, an **empty body** on failure
/// (never an error envelope, which the provider would save as a corrupt
/// subtitle), and detect-language as `200` JSON / `200`-`Unknown` on failure.
#[cfg(all(test, feature = "bazarr"))]
mod bazarr_routes_tests {
    use super::*;
    use axum::http::{HeaderMap, Request};
    use http_body_util::BodyExt;
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
        let bytes = resp.into_body().collect().await.unwrap().to_bytes().to_vec();
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
        assert!(headers
            .get(header::CONTENT_TYPE)
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("text/plain"));
        assert_eq!(String::from_utf8(body).unwrap(), SRT);
    }

    #[tokio::test]
    async fn asr_failure_returns_empty_body() {
        // Transcriber error → empty body, never an error envelope.
        let (status, _h, body) =
            post(with_fake(true), "/bazarr/asr?output=srt&encode=false", b"\x00\x01").await;
        assert_eq!(status, StatusCode::OK);
        assert!(body.is_empty(), "failure must be an empty body, got {body:?}");

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
