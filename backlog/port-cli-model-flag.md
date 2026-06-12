# UX: `--model` flag + helpful missing-model error

## what
Add `--model <PATH>` to `transcribe`. Resolve the model in order:
`--model` > config `whisper.model` (if a path) > `SUBMATE__WHISPER__MODEL`.
When none resolves, exit with a clear, non-panicking error naming both the flag
and the env var and pointing at a download (e.g. ggml-base.en.bin from
huggingface.co/ggerganov/whisper.cpp).

## where
`rust/crates/submate-cli/src/main.rs` — `TranscribeArgs` + a `resolve_model()`
helper + the error.

## why
Today the model is only settable via an obscure env var, with no flag and a
confusing failure when it's missing.

## falsifies
`cargo test -p submate-cli transcribe_model_resolution`: `--model` parses; the
resolver returns the documented error (a `Result::Err` with the flag+env+download
hint), not a panic, when nothing is configured. No real model needed.
