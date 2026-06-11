# Node Dispatcher: semaphore + spawn_blocking

**blocked-by:** port-whisper-pipeline

## what
The per-node execution core: `Semaphore(runners)` gating `tokio::task::spawn_blocking` calls into the submate-whisper pipeline. Caps concurrent transcriptions to the node's runner count; returns the result. See rust/docs/architecture.md.

## where
`rust/crates/submate-node/src/lib.rs`.

## why
Where transcription actually runs in the FileFlows topology — on the node, not the server. Uses Rust's in-process concurrency (the thing Python's queue worked around).

## falsifies
`cargo test -p submate-node dispatcher_caps_concurrency` — with runners=2, a third concurrent submit waits until a permit frees (observed via a barrier/counter), and results return correctly.
