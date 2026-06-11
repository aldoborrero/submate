# tried: port-queue-models-enums

## outcome
Abandoned — scope violation (denylist hit).

## what happened
The grind branch `grind/port-queue-models-enums` modified a file outside the
item's allowed scope:

- `rust/fixtures/capture/capture_enums.py`

This path is on the denylist. Capture inputs are golden parity data and must be
authored by a human/capture run, not by an automated port item: the item itself
calls for extending `rust/fixtures/types/enum_values.json` by adding
`OutputFormat`/`SkipReason` to `capture_enums.py`'s `ENUMS` list and re-running
capture. Because the porter touched the denylisted capture file, the branch
could not be auto-applied and was rejected.

## actions taken
- Removed worktree `port-queue-models-enums` and deleted branch
  `grind/port-queue-models-enums`.
- Restored `backlog/port-queue-models-enums.md` from `origin/main` and rerouted
  it to `backlog/needs-human/port-queue-models-enums.md`. Triage skips
  `backlog/` subdirectories, so the item will not be auto-picked again.

## next steps (human)
A human reviews `backlog/needs-human/port-queue-models-enums.md` and either:
1. applies the denylisted change directly (add `OutputFormat`/`SkipReason` to
   `rust/fixtures/capture/capture_enums.py` and re-run capture to regenerate
   `rust/fixtures/types/enum_values.json`), then re-runs the item, or
2. re-scopes the item to exclude the denylisted capture file and moves it back
   to `backlog/` for automated pickup, or
3. deletes the item if it is no longer wanted.
