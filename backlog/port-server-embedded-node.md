# Embedded in-process node

**blocked-by:** port-server-node-api, port-node-agent

## what
`submate server` runs an in-process node by default (FileFlows "Internal Node") so a single box works with no separate process; disableable via config for brain-only deployments.

## where
`rust/crates/submate-server/src/lib.rs` wiring a `submate-node` agent against the local API/queue.

## why
Zero-setup single-box usage while keeping the same pull-based path as remote nodes.

## falsifies
`cargo test -p submate-server embedded_node_drains` — a server with the embedded node enabled processes an enqueued job end-to-end (mock transcription) and marks it done.
