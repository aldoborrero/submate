# Port the server↔node wire types (submate-proto)

**blocked-by:** port-types-enums

## what
Define the serde types for the node-coordination protocol: NodeRegister {id, gpu, runners, tasks}, WorkRequest, WorkResponse {job_id, kind, audio_url, opts} | NoWork, Progress {pct}, JobResult {ok, output|error}, Heartbeat. See rust/docs/architecture.md.

## where
`rust/crates/submate-proto/src/lib.rs`. Pure crate — `serde` only, no I/O deps.

## why
Both submate-server and submate-node depend on these; a shared crate keeps the wire contract in one place.

## falsifies
`cargo test -p submate-proto roundtrip` serde round-trips every message type (serialize → deserialize → equal).
