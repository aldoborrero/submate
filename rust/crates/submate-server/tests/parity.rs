//! Core router parity falsifiers.
//!
//! The `root()` / `status()` handlers
//! (`rust/crates/submate-server/src/lib.rs`) reproduce the response shapes of
//! the Python `submate/server/handlers/core/router.py`. These tests pin them to
//! the captured golden `server/core_router.json` so the Rust handlers and the
//! Python SPEC cannot silently drift.
//!
//! * `core_router::root` — `GET /` must equal the golden `root` object exactly:
//!   `name`, `version`, `docs`, and all five `endpoints` keys/values
//!   (`bazarr_asr`, `bazarr_detect_language`, `jellyfin`, `status`, `queue`).
//! * `core_router::status` — `GET /status` must carry the static envelope from
//!   the golden `status` object (`status`/`version` scalars) plus a `queue`
//!   object key. The queue *contents* are deliberately not pinned: Python
//!   returns live Huey state (`{pending, scheduled}`) while the Rust server uses
//!   a node-topology shape on purpose (see `rust/docs/architecture.md`).

use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use http_body_util::BodyExt;
use parity::{assert_json_eq, golden};
use serde_json::Value;
use submate_server::{app, AppState};
use tower::ServiceExt;

async fn get_json(app: Router, uri: &str) -> (StatusCode, Value) {
    let res = app
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    (status, body)
}

/// Resolve the routed status of `(method, uri)` against a fresh `app`, sending an
/// empty body. A `404 NOT_FOUND` means the path is *not registered*; any other
/// status (including handler-level rejections like `415`/`422`/`503`) means the
/// route exists. We deliberately only care about routed-vs-unrouted here.
async fn route_status(method: &str, uri: &str) -> StatusCode {
    app(AppState::default())
        .oneshot(
            Request::builder()
                .method(method)
                .uri(uri)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
        .status()
}

/// Route-path contract guard. Route paths are a byte-for-byte contract with the
/// Python FastAPI app (`submate/server/handlers/**`): the Jellyfin webhook is
/// exposed at `POST /webhooks/jellyfin`, *not* `/jellyfin/webhook`. A future
/// implementer who trusts a drifted doc and registers `/jellyfin/webhook` would
/// silently 404 every real Jellyfin plugin POST. Pin both the positive contract
/// (the canonical paths are routed) and the negative guard (the wrong path is
/// not) so the drift cannot recur unnoticed.
#[cfg(feature = "jellyfin")]
#[tokio::test]
async fn routes() {
    // Positive contract: the canonical Jellyfin webhook path is registered.
    // An empty body trips a handler-level rejection (non-404), which is exactly
    // what we want — we are only distinguishing routed from unrouted here.
    let routed = route_status("POST", "/webhooks/jellyfin").await;
    assert_ne!(
        routed,
        StatusCode::NOT_FOUND,
        "POST /webhooks/jellyfin must be registered, but it 404'd"
    );

    // The always-on ops routes share the same byte-for-byte path contract.
    for &(method, uri) in &[("GET", "/"), ("GET", "/status"), ("GET", "/queue")] {
        let status = route_status(method, uri).await;
        assert_ne!(
            status,
            StatusCode::NOT_FOUND,
            "contract route {method} {uri} must be registered, but it 404'd"
        );
    }

    // Negative guard: the wrong Jellyfin path must NOT be routed. This is the
    // failure mode an implementer trusting a drifted doc would introduce.
    let wrong = route_status("POST", "/jellyfin/webhook").await;
    assert_eq!(
        wrong,
        StatusCode::NOT_FOUND,
        "POST /jellyfin/webhook must not be routed (canonical path is \
         /webhooks/jellyfin), got {wrong}"
    );
}

#[tokio::test]
async fn root() {
    let (status, body) = get_json(app(AppState::default()), "/").await;
    assert_eq!(status, StatusCode::OK);

    let expected = golden("server/core_router.json");
    assert_json_eq(&body, &expected["root"]);
}

#[tokio::test]
async fn status() {
    let (http_status, body) = get_json(app(AppState::default()), "/status").await;
    assert_eq!(http_status, StatusCode::OK);

    let expected = golden("server/core_router.json");
    let expected_status = &expected["status"];

    // Static scalar envelope is pinned to the golden.
    assert_eq!(body["status"], expected_status["status"]);
    assert_eq!(body["version"], expected_status["version"]);

    // The golden records only that a `queue` key is present (its live contents
    // are out of scope); assert the same here — a `queue` object must exist.
    assert_eq!(expected_status["queue_key_present"], Value::Bool(true));
    assert!(
        body.get("queue").is_some_and(Value::is_object),
        "GET /status must carry a `queue` object, got: {body}"
    );
}
