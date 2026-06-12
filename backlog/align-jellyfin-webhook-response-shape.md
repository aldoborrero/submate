# align: Jellyfin webhook response shape drifts from Python

**contract:** ROUTE SIGNATURE — `POST /webhooks/jellyfin` response JSON body.

## what differs

The current axum skeleton handler `jellyfin_webhook` in
`rust/crates/submate-server/src/lib.rs` returns a response shape that **never
appears anywhere in the Python SPEC**:

```rust
// rust/crates/submate-server/src/lib.rs, fn jellyfin_webhook
Ok(Json(json!({
    "status": "accepted",
    "notification_type": payload.notification_type,
    "item_id": payload.item_id,
})))
```

Python (`submate/server/handlers/jellyfin/handlers.py::handle_jellyfin_webhook`,
returned verbatim by the route in `.../jellyfin/router.py`) only ever returns one
of three shapes:

| condition | body |
|-----------|------|
| event not configured for processing (`process_on_add`/`process_on_play` gate) | `{"status": "skipped", "message": "Event {notification_type} not configured"}` |
| enqueue succeeds | `{"status": "queued", "task_id": <ItemId>, "file_path": <mapped_path>}` |
| processing raises | `{"status": "error", "message": <str(exc)>}` |

Divergences, all byte-observable on the wire:

1. **`status` literal** — Python emits `"skipped" | "queued" | "error"`. The Rust
   skeleton emits `"accepted"`, which is not a Python value.
2. **response keys** — Python uses `message` / `task_id` / `file_path`. The Rust
   skeleton uses `notification_type` / `item_id`, which appear in **no** Python
   response.
3. **`task_id` provenance** — when queued, Python sets `task_id` to the *ItemId*
   (see the inline `# Use ItemId as task reference`), not a queue/job id. The port
   must preserve this even though the Rust queue model has its own job ids.

The 400 path (bad/absent `User-Agent` not containing `Jellyfin-Server`) is already
correct: Python raises `HTTPException(400, "Invalid request - not from Jellyfin
server")`; the Rust skeleton returns `ServerError::BadRequest` with the same
message. No change needed there.

## why it matters

This is a route response-shape contract: a Jellyfin webhook caller (and any
integration test) sees the JSON body. The skeleton was landed as a placeholder
(its doc-comment defers behavior to `backlog/port-server-jellyfin-webhook.md`),
but it picked a foreign response shape an implementer can carry forward. The
existing port item specifies the enqueue *behavior* but does not pin the response
*body*, so this needs an explicit alignment note.

## where

- Rust: `rust/crates/submate-server/src/lib.rs` — `fn jellyfin_webhook`.
- Python SPEC: `submate/server/handlers/jellyfin/handlers.py`
  (`handle_jellyfin_webhook`) + `.../jellyfin/router.py`.
- Folds into: `backlog/port-server-jellyfin-webhook.md` (add the response-shape
  contract to its acceptance criteria).

## falsifies

`cargo test -p submate-server jellyfin_webhook_response_shape` — a POST with a
valid `Jellyfin-Server` User-Agent and an `ItemAdded` payload, against a server
configured with `process_on_add = false` (the deterministic, node-free path),
must return:

```json
{"status": "skipped", "message": "Event ItemAdded not configured"}
```

with exactly those two keys. The current handler fails this: it returns
`"status": "accepted"` plus `notification_type` / `item_id`, and omits `message`.

A second case pins the success shape once enqueue is wired: with
`process_on_add = true` and a (mock) node/queue, the body is
`{"status": "queued", "task_id": "<ItemId>", "file_path": "<path>"}` — `task_id`
equals the request's `ItemId`, not an internal job id.
