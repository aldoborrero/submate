# tried: port-whisper-pipeline

## outcome
Abandoned — scope violation (denylist hit).

## what happened
The grind branch `grind/port-whisper-pipeline` modified a file outside the
item's allowed scope:

- `rust/fixtures/transcribe/clipA.expected.srt`

This path is on the denylist. Files under `rust/fixtures/**` are the goldens
the Rust ports are diffed against, so an automated porter authoring one would
let the implementation define its own oracle (the parity falsifier would pass
vacuously, asserting Rust output against Rust-authored "goldens"). The item's
stated scope was the pipeline entry point
`rust/crates/submate-whisper/src/lib.rs` (plus its parity test), which asserts
against `rust/fixtures/transcribe/*.segments.json` — explicitly not the
expected-SRT golden. Because the porter touched the denylisted fixture, the
branch could not be auto-applied and was rejected.

## actions taken
- Removed worktree `port-whisper-pipeline` and deleted branch
  `grind/port-whisper-pipeline`.
- Restored `backlog/port-whisper-pipeline.md` from `origin/main` and rerouted
  it to `backlog/needs-human/port-whisper-pipeline.md`. Triage skips `backlog/`
  subdirectories, so the item will not be auto-picked again.

## next steps (human)
A human reviews `backlog/needs-human/port-whisper-pipeline.md` and either:
1. applies the denylisted change directly (author the
   `rust/fixtures/transcribe/clipA.expected.srt` golden via the capture
   harness / faster-whisper reference run, not by hand from Rust output), then
   re-runs the item, or
2. re-scopes the item to exclude the denylisted fixture and moves it back to
   `backlog/` for automated pickup, or
3. deletes the item if it is no longer wanted.
