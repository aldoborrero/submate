# Node-coordination API (register / request-work / progress / result / heartbeat)

**blocked-by:** port-queue-store, port-proto-types, port-server-app-ops

## what
axum routes: `POST /nodes/register`, `POST /nodes/{id}/request-work` (long-poll → atomic claim or 204), `POST /jobs/{id}/progress`, `POST /jobs/{id}/result` (marks done, wakes any synchronous waiter), `POST /nodes/{id}/heartbeat` (extends lease). See rust/docs/architecture.md.

## where
`rust/crates/submate-server/src/lib.rs` using `submate-proto` types over `submate-queue`.

## why
The pull-based work-distribution core of the FileFlows/Unmanic topology.

## falsifies
`cargo test -p submate-server node_api_roundtrip` — register a node; enqueue a job; request-work returns it; posting a result marks it done; a heartbeat extends the lease (a non-heartbeating node's job is reclaimed).
