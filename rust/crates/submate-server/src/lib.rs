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

use std::sync::Arc;

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use serde::Serialize;
use serde_json::json;

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
#[derive(Clone)]
pub struct AppState {
    stats: Arc<dyn StatsSource>,
}

impl AppState {
    /// Build state from a [`StatsSource`].
    pub fn new(stats: impl StatsSource) -> AppState {
        AppState {
            stats: Arc::new(stats),
        }
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
}

impl ServerError {
    fn status(&self) -> StatusCode {
        match self {
            ServerError::NotFound(_) => StatusCode::NOT_FOUND,
            ServerError::BadRequest(_) => StatusCode::BAD_REQUEST,
            ServerError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
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

/// Build the full server [`Router`], mounting the always-on ops routes plus any
/// feature-enabled integration routers.
pub fn app(state: AppState) -> Router {
    let router = ops_router();

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

/// Placeholder jellyfin router; routes are added by the jellyfin port item.
#[cfg(feature = "jellyfin")]
fn jellyfin_router() -> Router<AppState> {
    Router::new()
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

    #[test]
    fn server_error_renders_json_envelope_with_status() {
        let res = ServerError::NotFound("nope".into()).into_response();
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
        let res = ServerError::BadRequest("bad".into()).into_response();
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
        let res = ServerError::Internal("boom".into()).into_response();
        assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }
}
