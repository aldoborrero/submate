# Jellyfin webhook route (enqueue)

**blocked-by:** port-server-node-api, port-queue-transcription-service

## what
Port `POST /webhooks/jellyfin` (router prefix `/webhooks`, path `/jellyfin`, per `submate/server/handlers/jellyfin/router.py` — NOT `/jellyfin/webhook`; see `backlog/align-jellyfin-webhook-route.md`): validate User-Agent, filter ItemAdded, resolve the file path, run the skip decision, and enqueue a file-transcription job (fire-and-forget; a node drains it).

## where
`rust/crates/submate-server/src/lib.rs`.

## why
The Jellyfin auto-transcription trigger, now an enqueue into the central queue.

## response shape (contract)
The route returns Python `handle_jellyfin_webhook`'s shapes verbatim — never the
foreign `{status: accepted, ...}`:

| condition | body |
|-----------|------|
| event not configured (`process_on_add` / `process_on_play` gate) | `{"status": "skipped", "message": "Event {notification_type} not configured"}` |
| enqueue succeeds | `{"status": "queued", "task_id": <ItemId>, "file_path": <mapped_path>}` |
| processing raises | `{"status": "error", "message": <str(exc)>}` |

`task_id` is the *ItemId* (Python's `# Use ItemId as task reference`), not an
internal queue/job id. The skipped path is already wired (see
`jellyfin_webhook_response_shape` in `rust/crates/submate-server/src/lib.rs`); the
should-process branch currently returns the `error` shape and is replaced by the
queued/error wiring below.

## falsifies
`cargo test -p submate-server jellyfin_webhook` — the sample ItemAdded payload enqueues exactly one file-transcription job (and is skipped when a skip condition holds). With `process_on_add = true` and a mock node/queue, the body is `{"status": "queued", "task_id": "<ItemId>", "file_path": "<path>"}`.
