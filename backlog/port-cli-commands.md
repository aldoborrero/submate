# Port the clap CLI

**blocked-by:** port-config-validators, port-whisper-pipeline, port-translate-srt-apply, port-node-agent

## what
Port the `submate` CLI: subcommands transcribe / translate / server / node / config, the global `--config-file`, and tracing-based logging setup. The old `worker` command is replaced by `node` (`submate node --server <url>` runs a remote processing node; `server` runs an embedded node by default). `transcribe --sync` runs a one-shot local node + dispatcher.

## where
`rust/crates/submate-cli/src/main.rs`. Use `clap` + `tracing-subscriber` (workspace deps).

## why
The user-facing entry point wiring the server + node binaries together.

## falsifies
`cargo test -p submate-cli cli_help` asserts the subcommand set (incl. `node`, not `worker`) + flags, and `submate config` prints resolved config equal to `rust/fixtures/config/defaults.resolved.json`.
