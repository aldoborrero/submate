# tried: port-server-app-ops

## outcome
Abandoned — scope violation (denylist hit).

## what happened
The grind branch `grind/port-server-app-ops` modified a file outside the item's
allowed scope:

- `rust/fixtures/server/ops.json`

This path is on the denylist. Server fixtures are golden parity data and must be
authored by a human/capture run, not by an automated port item: `ops.json` is
the expected `/version` and `/queue/stats` JSON that `cargo test -p
submate-server ops_routes` asserts against. A port item that edits its own
falsification fixture defeats the parity check, so the branch could not be
auto-applied and was rejected.

## actions taken
- Removed worktree `port-server-app-ops` and deleted branch
  `grind/port-server-app-ops`.
- Restored `backlog/port-server-app-ops.md` from `origin/main` and rerouted it
  to `backlog/needs-human/port-server-app-ops.md`. Triage skips `backlog/`
  subdirectories, so the item will not be auto-picked again.

## next steps (human)
A human reviews `backlog/needs-human/port-server-app-ops.md` and either:
1. applies the denylisted change directly (author the expected `/version` and
   `/queue/stats` JSON in `rust/fixtures/server/ops.json`), then re-runs the
   item, or
2. re-scopes the item to exclude the denylisted fixture file and moves it back
   to `backlog/` for automated pickup, or
3. deletes the item if it is no longer wanted.
