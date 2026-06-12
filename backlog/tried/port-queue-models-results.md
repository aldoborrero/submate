# tried: port-queue-models-results

## outcome
Abandoned — scope violation (denylist hit).

## what happened
The grind branch `grind/port-queue-models-results` modified a file outside the
item's allowed scope:

- `rust/fixtures/capture/capture_queue.py`

This path is on the denylist. Capture inputs are golden parity data and must be
authored by a human/capture run, not by an automated port item: the item itself
calls for capturing the five canonical task-envelope JSON objects into
`rust/fixtures/queue/task_envelopes.json` (no `rust/fixtures/queue/` dir exists
yet) by adding/driving a capture script `capture_queue.py`. Because the porter
touched the denylisted capture file, the branch could not be auto-applied and
was rejected.

## actions taken
- Removed worktree `port-queue-models-results` and deleted branch
  `grind/port-queue-models-results`.
- Restored `backlog/port-queue-models-results.md` from `origin/main` and
  rerouted it to `backlog/needs-human/port-queue-models-results.md`. Triage
  skips `backlog/` subdirectories, so the item will not be auto-picked again.

## next steps (human)
A human reviews `backlog/needs-human/port-queue-models-results.md` and either:
1. applies the denylisted change directly (author/run
   `rust/fixtures/capture/capture_queue.py` to emit the five canonical envelope
   JSON objects into `rust/fixtures/queue/task_envelopes.json`), then re-runs
   the item, or
2. re-scopes the item to exclude the denylisted capture file and moves it back
   to `backlog/` for automated pickup, or
3. deletes the item if it is no longer wanted.
