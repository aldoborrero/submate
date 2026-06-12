# Server-side transcription decision + result handling (9 skip conditions)

**blocked-by:** port-subtitle-detect, port-whisper-pipeline

## what
Port `TranscriptionService`: the 9 skip conditions and audio-track selection that decide whether to ENQUEUE a file job (now server-side at ingestion), plus the post-result subtitle write (atomic temp + rename) when a node returns the SRT. The transcription itself runs on a node, not here.

## where
`rust/crates/submate-queue/src/lib.rs` (decision/enqueue) + `submate-server` (write on result).

## why
The skip logic is faithful business logic (parity), re-homed to the ingestion side of the server.

## falsifies
`cargo test -p submate-queue parity::skip_conditions` reproduces each of the 9 skip outcomes against `rust/fixtures/queue/skip_cases.json`.
