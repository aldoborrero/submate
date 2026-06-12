# tried: port-stablets-regroup-parse-B1

## outcome
Abandoned — scope violation (denylist hit).

## what happened
The grind branch `grind/port-stablets-regroup-parse-B1` modified a file outside
the item's allowed scope:

- `rust/fixtures/stablets/regroup_parse.json`

This path is on the denylist. The `regroup_parse.json` golden is parity capture
data and must be authored by a human/capture run, not by an automated port item:
the item's falsifier (`cargo test -p stable-ts parity::regroup_parse`) is meant
to *produce/compare against* that golden, not have the porter hand-write it.
Because the porter touched the denylisted fixture, the branch could not be
auto-applied and was rejected.

## actions taken
- Removed worktree `port-stablets-regroup-parse-B1` and deleted branch
  `grind/port-stablets-regroup-parse-B1`.
- Restored `backlog/port-stablets-regroup-parse-B1.md` from `origin/main` and
  rerouted it to `backlog/needs-human/port-stablets-regroup-parse-B1.md`. Triage
  skips `backlog/` subdirectories, so the item will not be auto-picked again.

## next steps (human)
A human reviews `backlog/needs-human/port-stablets-regroup-parse-B1.md` and
either:
1. applies the denylisted change directly (capture the `regroup_parse.json`
   op-list golden), then re-runs the item, or
2. re-scopes the item to exclude the denylisted fixture and moves it back to
   `backlog/` for automated pickup, or
3. deletes the item if it is no longer wanted.
