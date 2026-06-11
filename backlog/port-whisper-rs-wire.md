# Wire whisper-rs into submate-whisper

**blocked-by:** port-stablets-model-A

## what
Add `whisper-rs` (workspace dep), load a model, run it on a PCM f32 clip in `spawn_blocking`, request word-level timestamps, and build a `stable-ts::WhisperResult`.

## where
`rust/crates/submate-whisper/src/lib.rs`. Requires LIBCLANG_PATH/cmake (provided by the devshell).

## why
Native inference replacing faster-whisper; supplies the word timestamps the stable-ts slice needs.

## falsifies
`cargo test -p submate-whisper transcribe_smoke` (model gated behind a feature/env) produces a non-empty WhisperResult with per-word timings for `rust/fixtures/stablets/clipA/audio.f32`.
