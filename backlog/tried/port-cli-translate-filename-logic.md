# tried: port-cli-translate-filename-logic

## outcome
Abandoned — scope violation (denylist hit).

## what happened
The grind branch `grind/port-cli-translate-filename-logic` modified a file
outside the item's allowed scope:

- `rust/fixtures/capture/capture_cli_translate.py`

This path is on the denylist. Capture scripts under `rust/fixtures/capture/`
produce the Python goldens the Rust ports are diffed against, so an automated
porter editing them would let the implementation define its own oracle. The
item's stated scope was the pure-data helper module
`rust/crates/submate-cli/src/translate_paths.rs` (plus its parity test and the
JSON fixture it consumes) — explicitly not the capture harness. Because the
porter touched the denylisted capture file, the branch could not be
auto-applied and was rejected.

## actions taken
- Removed worktree `port-cli-translate-filename-logic` and deleted branch
  `grind/port-cli-translate-filename-logic`.
- Restored `backlog/port-cli-translate-filename-logic.md` from `origin/main`
  and rerouted it to
  `backlog/needs-human/port-cli-translate-filename-logic.md`. Triage skips
  `backlog/` subdirectories, so the item will not be auto-picked again.

## next steps (human)
A human reviews `backlog/needs-human/port-cli-translate-filename-logic.md`
and either:
1. applies the denylisted change directly (author/run the
   `rust/fixtures/capture/capture_cli_translate.py` change and re-run capture to
   regenerate the affected `rust/fixtures/cli/` goldens), then re-runs the item, or
2. re-scopes the item to exclude the denylisted capture file and moves it back
   to `backlog/` for automated pickup, or
3. deletes the item if it is no longer wanted.
