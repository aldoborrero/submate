# align: server HTTP error bodies use `{"error": ...}`, Python/FastAPI uses `{"detail": ...}`

**contract:** ROUTE SIGNATURE — error response JSON bodies for the server
routes must match the FastAPI SPEC byte-for-byte. Every `HTTPException` in
`submate/server/` and the global exception handler emit the FastAPI envelope
`{"detail": "<message>"}`. The Rust server emits `{"error": "<message>"}` for
every error variant. The top-level key differs (`detail` vs `error`), so a
Bazarr/Jellyfin client parsing the error body sees a different shape than it
sees from the Python server.

## the divergence

Python (the SPEC) — every server error body is FastAPI's `{"detail": ...}`:

```python
# submate/server/handlers/jellyfin/router.py
raise HTTPException(status_code=400, detail="Invalid request - not from Jellyfin server")
raise HTTPException(status_code=500, detail=str(e))

# submate/server/handlers/bazarr/router.py
raise HTTPException(status_code=400, detail=str(e))
raise HTTPException(status_code=500, detail="Transcription failed")

# submate/server/server.py  (global handler)
return JSONResponse(status_code=500, content={"detail": "Internal server error"})
```

FastAPI serializes `HTTPException(detail=X)` as the JSON object `{"detail": X}`
with the given status code. So a 400 from the Jellyfin webhook is exactly:

```json
{"detail": "Invalid request - not from Jellyfin server"}
```

Rust (`rust/crates/submate-server/src/lib.rs`, `impl IntoResponse for
ServerError`) renders **every** variant as:

```rust
(status, Json(json!({ "error": self.to_string() }))).into_response()
```

i.e. `{"error": "<message>"}`. The doc comment above the enum even codifies the
wrong shape as the intended envelope:

```rust
/// Every variant maps to a JSON body `{"error": "<message>"}` plus an HTTP
/// status, so clients see a single, predictable error envelope ...
```

The status codes line up (400/404/500/503), but the **body key** does not.

## falsifier

The deterministic, no-node path is `POST /webhooks/jellyfin` with a non-Jellyfin
`User-Agent`: Python's router rejects it with `HTTPException(status_code=400,
detail="Invalid request - not from Jellyfin server")` *before* any queue/node
interaction, and the Rust `jellyfin_webhook` rejects it via
`ServerError::BadRequest(...)` on the same condition. Same status, different body.

Add to `rust/crates/submate-server/src/lib.rs` tests (or `tests/parity.rs`),
extending the existing `jellyfin_webhook_rejects_non_jellyfin_user_agent` which
today asserts only the status:

```rust
// curl/8.0 user-agent -> 400. Python emits {"detail": "..."}, the FastAPI
// HTTPException envelope. This currently FAILS: Rust emits {"error": "..."}.
assert_eq!(res.status(), StatusCode::BAD_REQUEST);
let body: serde_json::Value = /* parse res body */;
assert_eq!(
    body["detail"],
    "Invalid request - not from Jellyfin server",
    "FastAPI error bodies use the `detail` key, not `error`"
);
assert!(body.get("error").is_none(), "must not use the `error` key");
```

This assertion fails against the current `IntoResponse for ServerError`.

## fix

Change `impl IntoResponse for ServerError` to render `{"detail": self.to_string()}`
instead of `{"error": ...}`, and update the enum doc comment (the
`{"error": "<message>"}` line) to match. The string message for the 400 path is
already byte-correct (`"Invalid request - not from Jellyfin server"`), so only
the envelope key changes.

Note this is the FastAPI default envelope for `HTTPException` — it applies to
every error surfaced by the server (the future bazarr `/bazarr/asr` 400/500
paths in `port-server-bazarr-asr.md` and the `port-server-jellyfin-webhook.md`
500 path must land on `detail` too, not `error`). Pin the envelope here so those
ports inherit the correct shape rather than re-deriving it.

## scope

Server crate only (`rust/crates/submate-server/src/lib.rs`). The node/job routes
(`/nodes/*`, `/jobs/*`) are a Rust-only topology extension with no Python SPEC,
so their error bodies are not a parity case — but switching the shared
`ServerError` envelope to `detail` is harmless for them (single predictable
envelope is preserved; only the key name changes). Out of scope: the `/queue`
node-topology stats shape (`{pending,running,done,nodes}` vs Python
`{pending,scheduled}`), already declared an intentional re-architecture in
`backlog/parity-server-core-router-root-status.md`.
