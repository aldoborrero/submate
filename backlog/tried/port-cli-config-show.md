# tried: port-cli-config-show

## outcome
Abandoned — scope violation (denylist hit).

## what happened
The grind branch `grind/port-cli-config-show` modified a file outside the
item's allowed scope:

- `rust/fixtures/capture/capture_cli_config.py`

This path is on the denylist. Capture scripts under `rust/fixtures/capture/`
produce the Python goldens the Rust ports are diffed against, so an automated
porter editing them would let the implementation define its own oracle. Because
the porter touched the denylisted capture file, the branch could not be
auto-applied and was rejected.

## actions taken
- Removed worktree `port-cli-config-show` and deleted branch
  `grind/port-cli-config-show`.
- Restored `backlog/port-cli-config-show.md` from `origin/main` and rerouted it
  to `backlog/needs-human/port-cli-config-show.md`. Triage skips `backlog/`
  subdirectories, so the item will not be auto-picked again.

## next steps (human)
A human reviews `backlog/needs-human/port-cli-config-show.md` and either:
1. applies the denylisted change directly (author/run the
   `rust/fixtures/capture/capture_cli_config.py` change and re-run capture to
   regenerate the affected `rust/fixtures/cli/` goldens), then re-runs the item, or
2. re-scopes the item to exclude the denylisted capture file and moves it back
   to `backlog/` for automated pickup, or
3. deletes the item if it is no longer wanted.
