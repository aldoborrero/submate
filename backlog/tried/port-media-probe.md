# tried: port-media-probe

## outcome
Abandoned — scope violation (denylist hit).

## what happened
The implementer branch `grind/port-media-probe` modified a file outside the
item's allowed scope:

- `rust/fixtures/media/sample.probe.json`

This path is on the denylist (fixtures are golden parity data and must not be
authored or altered by an automated port item), so the branch was rejected
rather than merged.

## actions taken
- Removed worktree `port-media-probe` and deleted branch
  `grind/port-media-probe`.
- Rerouted the backlog item to `backlog/needs-human/port-media-probe.md`.
  Triage skips subdirectories, so the item will not be auto-picked again.

## next steps (human)
A human reviews `backlog/needs-human/port-media-probe.md` and either:
1. applies the denylisted change (the fixture `sample.probe.json`) directly,
   then re-runs the item, or
2. re-scopes the item to exclude the denylisted path and moves it back to
   `backlog/` for automated pickup, or
3. deletes the item if it is no longer wanted.
