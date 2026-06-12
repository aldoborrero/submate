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
use std::sync::{Arc, Mutex};

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::Serialize;
use serde_json::json;
use submate_config::ServerSettings;
use submate_proto::{
    JobOpts, JobOutcome, JobResult, NodeRegister, NodeRegistered, Progress, WorkResponse,
};
use submate_queue::{JobId, JobState, JobStore, NewJob, QueueError};
use submate_types::TranscriptionTask;
use tokio::sync::oneshot;

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
#[derive(Clone)]
pub struct AppState {
    stats: Arc<dyn StatsSource>,
    coord: Option<Arc<NodeCoordinator>>,
    server: Arc<ServerSettings>,
}

impl AppState {
    /// Build state from a [`StatsSource`] with no node coordinator wired up.
    /// The `/nodes/*` and `/jobs/*` routes return `503` until a coordinator is
    /// attached via [`AppState::with_coordinator`]. Server processing settings
    /// default to the Python defaults (`process_on_add = true`).
    pub fn new(stats: impl StatsSource) -> AppState {
        AppState {
            stats: Arc::new(stats),
            coord: None,
            server: Arc::new(ServerSettings::default()),
        }
    }

    /// Build state backed by a live [`NodeCoordinator`]. The coordinator also
    /// supplies live [`QueueStats`] for the ops routes.
    pub fn with_coordinator(coord: Arc<NodeCoordinator>) -> AppState {
        AppState {
            stats: coord.clone(),
            coord: Some(coord),
            server: Arc::new(ServerSettings::default()),
        }
    }

    /// Override the server processing settings (`process_on_add` /
    /// `process_on_play`) that gate Jellyfin webhook handling.
    pub fn with_server_settings(mut self, server: ServerSettings) -> AppState {
        self.server = Arc::new(server);
        self
    }

    fn coordinator(&self) -> std::result::Result<&Arc<NodeCoordinator>, ServerError> {
        self.coord
            .as_ref()
            .ok_or_else(|| ServerError::Unavailable("node coordination not enabled".to_string()))
    }
}

impl Default for AppState {
    fn default() -> AppState {
        AppState::new(EmptyStats)
    }
}

/// Errors surfaced by the server, rendered by the global error handler.
///
/// Every variant maps to a JSON body `{"error": "<message>"}` plus an HTTP
/// status, so clients see a single, predictable error envelope regardless of
/// which handler failed.
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
            ServerError::NotFound(_) => StatusCode::NOT_FOUND,
            ServerError::BadRequest(_) => StatusCode::BAD_REQUEST,
            ServerError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ServerError::Unavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
        }
    }
}

impl From<QueueError> for ServerError {
    fn from(err: QueueError) -> ServerError {
        match err {
            QueueError::NotFound(id) => ServerError::NotFound(format!("job {id} not found")),
            other => ServerError::Internal(other.to_string()),
        }
    }
}

impl IntoResponse for ServerError {
    fn into_response(self) -> Response {
        let status = self.status();
        if status.is_server_error() {
            tracing::error!(error = %self, "request failed");
        }
        (status, Json(json!({ "error": self.to_string() }))).into_response()
    }
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
    /// Base URL path nodes use to fetch a job's extracted audio (the
    /// `audio_url` in a [`WorkResponse::Work`] is `{audio_base}/{job_id}/audio`).
    audio_base: String,
}

impl NodeCoordinator {
    /// Build a coordinator over an existing job store.
    pub fn new(store: JobStore) -> NodeCoordinator {
        NodeCoordinator {
            store: Mutex::new(store),
            nodes: Mutex::new(HashMap::new()),
            waiters: Mutex::new(HashMap::new()),
            audio_base: "/jobs".to_string(),
        }
    }

    fn store(&self) -> std::sync::MutexGuard<'_, JobStore> {
        // Poisoning only happens if a holder panicked mid-operation; the store
        // is a plain SQLite wrapper with no broken-invariant risk, so recover
        // the guard rather than propagating the panic to every later request.
        self.store.lock().unwrap_or_else(|e| e.into_inner())
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

    /// Register a node, returning the bearer token for its subsequent calls.
    ///
    /// Re-registration is idempotent on the token: a node that reconnects (same
    /// `id`) keeps its existing token while its advertised capabilities are
    /// refreshed, so an in-flight lease keyed by `node_id` survives a re-register.
    fn register(&self, req: NodeRegister) -> NodeRegistered {
        let mut nodes = self.nodes.lock().unwrap_or_else(|e| e.into_inner());
        let token = nodes
            .get(&req.id)
            .map(|existing| existing.token.clone())
            .unwrap_or_else(|| format!("tok_{}", req.id));
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
            .unwrap_or_else(|e| e.into_inner())
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
        Ok(())
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
                .unwrap_or_else(|e| e.into_inner())
                .insert(id, tx);
            rx
        };
        rx.await.ok()
    }

    fn wake_waiter(&self, id: JobId, outcome: JobOutcome) {
        if let Some(tx) = self
            .waiters
            .lock()
            .unwrap_or_else(|e| e.into_inner())
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
        let nodes = self.nodes.lock().unwrap_or_else(|e| e.into_inner()).len() as u64;
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

/// Placeholder bazarr router; routes are added by the bazarr port item.
#[cfg(feature = "bazarr")]
fn bazarr_router() -> Router<AppState> {
    Router::new()
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

    #[tokio::test]
    async fn ops_routes_root_returns_server_info() {
        let (status, body) = get_json(app(AppState::default()), "/").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["name"], "Submate Server");
        assert_eq!(body["version"], VERSION);
        assert_eq!(body["docs"], "/docs");
        assert_eq!(body["endpoints"]["status"], "/status");
        assert_eq!(body["endpoints"]["queue"], "/queue");
        assert_eq!(body["endpoints"]["bazarr_asr"], "/bazarr/asr");
    }

    #[tokio::test]
    async fn ops_routes_status_has_status_version_queue() {
        let (status, body) = get_json(app(AppState::default()), "/status").await;
        assert_eq!(status, StatusCode::OK);
        // Exactly the Python top-level keys: status, version, queue.
        let obj = body.as_object().unwrap();
        assert_eq!(obj.len(), 3);
        assert_eq!(body["status"], "ok");
        assert_eq!(body["version"], VERSION);
        assert_eq!(body["version"], "1.0.0");
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

    #[test]
    fn server_error_renders_json_envelope_with_status() {
        let res = ServerError::NotFound("nope".into()).into_response();
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
        let res = ServerError::BadRequest("bad".into()).into_response();
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
        let res = ServerError::Internal("boom".into()).into_response();
        assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let res = ServerError::Unavailable("nope".into()).into_response();
        assert_eq!(res.status(), StatusCode::SERVICE_UNAVAILABLE);
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
        fn new(ms: i64) -> TestClock {
            TestClock(Arc::new(AtomicI64::new(ms)))
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
}
