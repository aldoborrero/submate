# Jellyfin webhook route (enqueue)

**blocked-by:** port-server-node-api, port-queue-transcription-service

## what
Port `POST /webhooks/jellyfin` (router prefix `/webhooks`, path `/jellyfin`, per `submate/server/handlers/jellyfin/router.py` — NOT `/jellyfin/webhook`; see `backlog/align-jellyfin-webhook-route.md`): validate User-Agent, filter ItemAdded, resolve the file path, run the skip decision, and enqueue a file-transcription job (fire-and-forget; a node drains it).

## where
`rust/crates/submate-server/src/lib.rs`.

## why
The Jellyfin auto-transcription trigger, now an enqueue into the central queue.

## falsifies
`cargo test -p submate-server jellyfin_webhook` — the sample ItemAdded payload enqueues exactly one file-transcription job (and is skipped when a skip condition holds).
