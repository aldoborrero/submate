# tried: port-subtitle-discovery-fs

## outcome
Abandoned — scope violation (denylist hit).

## what happened
The grind branch `grind/port-subtitle-discovery-fs` modified a file outside the
item's allowed scope:

- `rust/fixtures/capture/capture_subtitle_discovery.py`

This path is on the denylist. Capture inputs are golden parity data and must be
authored by a human/capture run, not by an automated port item: capture scripts
under `rust/fixtures/capture/` produce the Python goldens the Rust ports are
diffed against, so an automated porter editing them would let the implementation
define its own oracle. Because the porter touched the denylisted capture file,
the branch could not be auto-applied and was rejected.

## actions taken
- Removed worktree `port-subtitle-discovery-fs` and deleted branch
  `grind/port-subtitle-discovery-fs`.
- Restored `backlog/port-subtitle-discovery-fs.md` from `origin/main` and
  rerouted it to `backlog/needs-human/port-subtitle-discovery-fs.md`. Triage
  skips `backlog/` subdirectories, so the item will not be auto-picked again.

## next steps (human)
A human reviews `backlog/needs-human/port-subtitle-discovery-fs.md` and either:
1. applies the denylisted change directly (author/run the
   `rust/fixtures/capture/capture_subtitle_discovery.py` change and re-run
   capture to regenerate the affected `rust/fixtures/` goldens), then re-runs
   the item, or
2. re-scopes the item to exclude the denylisted capture file and moves it back
   to `backlog/` for automated pickup, or
3. deletes the item if it is no longer wanted.
