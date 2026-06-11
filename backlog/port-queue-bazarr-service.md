# Bazarr ingestion: enqueue high-priority + await result

**blocked-by:** port-queue-store, port-whisper-pipeline

## what
Port `BazarrService` to the topology: detect-language / ASR become high-priority jobs the server enqueues and awaits (a node processes, posts the result, the server returns it to Bazarr). Output formatting (SRT/VTT/TXT/JSON) + translate-if-needed stay faithful.

## where
`rust/crates/submate-queue/src/lib.rs` / `submate-bazarr`.

## why
Keeps Bazarr's synchronous contract while routing the work through the pull-based node system.

## falsifies
`cargo test -p submate-queue bazarr_enqueue_await` — an ASR request enqueues a high-priority job and resolves to the node-posted result; output formatting matches `rust/fixtures/queue/bazarr.json`.
