# UX: silence whisper.cpp/ggml stderr spam

## what
whisper.cpp prints `whisper_full_with_state`, `seek=…`, `ggml_…`, `system_info`
lines straight to stderr on every transcribe — terminal noise. Install
whisper-rs's logging redirection at node/model init so those go through
`tracing` (hidden at the default INFO level, shown only at `--log-level DEBUG`).

## where
`rust/crates/submate-node/src/lib.rs` (model-feature init) — call
`whisper_rs::install_logging_hooks()` (or `set_log_callback`) once and route to
`tracing::debug!`. No raw `eprintln`/C stderr at default level.

## why
The single most visible CLI annoyance — every run floods the terminal.

## falsifies
`cargo test -p submate-node whisper_logging_hooked` asserts the model-init path
installs the whisper-rs logging hook exactly once (structural wiring; full
silence is confirmed by a human run, but the hook install is what's pinned). No
model file needed.
