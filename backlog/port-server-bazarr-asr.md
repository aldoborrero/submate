# Bazarr ASR routes (enqueue + await)

**blocked-by:** port-server-node-api, port-queue-bazarr-service, port-server-audio-transfer

## what
Port `/bazarr/asr` and `/bazarr/detect-language`: accept the raw s16le-PCM body (axum `Bytes`), relay it as the job audio payload, enqueue high-priority, and hold the response until a node returns the result (with timeout).

## where
`rust/crates/submate-server/src/lib.rs`.

## why
The Bazarr Whisper-provider integration, adapted from direct transcription to enqueue-and-await.

## falsifies
`cargo test -p submate-server bazarr_asr_enqueues` — posting a PCM body enqueues a high-priority job whose audio is fetchable; once a (mock) node posts the result, the HTTP response returns it.
