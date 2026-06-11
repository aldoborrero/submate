# tried: port-stablets-model-A

## outcome
Abandoned — scope violation (denylist hit).

## what happened
The implementer branch `grind/port-stablets-model-A` modified a file outside the
item's allowed scope:

- `rust/fixtures/capture/capture_stablets_model.py`

This path is on the denylist (capture/fixture-generation tooling must not be
authored or altered by an automated port item), so the branch was rejected
rather than merged.

## actions taken
- Removed worktree `port-stablets-model-A` and deleted branch
  `grind/port-stablets-model-A`.
- Rerouted the backlog item to `backlog/needs-human/port-stablets-model-A.md`.
  Triage skips subdirectories, so the item will not be auto-picked again.

## next steps (human)
A human reviews `backlog/needs-human/port-stablets-model-A.md` and either:
1. applies the denylisted change (the capture script update) directly, then
   re-runs the item, or
2. re-scopes the item to exclude the denylisted path and moves it back to
   `backlog/` for automated pickup, or
3. deletes the item if it is no longer wanted.
