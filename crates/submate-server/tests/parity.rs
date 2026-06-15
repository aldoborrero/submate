//! Core router parity falsifiers.
//!
//! The `root()` / `status()` handlers (`crates/submate-server/src/lib.rs`)
//! produce the response shapes of the core router. These tests pin them to the
//! golden `server/core_router.json` so the handlers cannot silently drift.
//!
//! * `core_router::root` — `GET /` must equal the golden `root` object exactly:
//!   `name`, `version`, `docs`, and all three `endpoints` keys/values
//!   (`bazarr_asr`, `bazarr_detect_language`, `status`).
//! * `core_router::status` — `GET /status` must equal the golden `status`
//!   envelope (`status`/`version`).

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use fixtures::{assert_json_eq, golden};
use http_body_util::BodyExt;
use serde_json::Value;
use submate_server::{AppState, app};
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
    assert_json_eq(&body, &expected["status"]);
}
