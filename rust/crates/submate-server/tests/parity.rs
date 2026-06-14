//! Core router parity falsifiers.
//!
//! The `root()` / `status()` handlers
//! (`rust/crates/submate-server/src/lib.rs`) reproduce the response shapes of
//! the Python `submate/server/handlers/core/router.py`. These tests pin them to
//! the captured golden `server/core_router.json` so the Rust handlers and the
//! Python SPEC cannot silently drift.
//!
//! * `core_router::root` — `GET /` must equal the golden `root` object exactly:
//!   `name`, `version`, `docs`, and all four `endpoints` keys/values
//!   (`bazarr_asr`, `bazarr_detect_language`, `status`, `queue`).
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
