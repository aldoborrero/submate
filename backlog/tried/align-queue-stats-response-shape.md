# tried: align-queue-stats-response-shape

**Outcome:** Abandoned — scope violation (denylist hit).

## What happened

The grind attempt modified a file outside the allowed scope for this item:

- `rust/fixtures/server/core_router.json`

This file is on the denylist, so the change could not be auto-applied and the
branch/worktree were discarded.

## Disposition

- Worktree `align-queue-stats-response-shape` removed.
- Branch `grind/align-queue-stats-response-shape` deleted.
- Item rerouted to `backlog/needs-human/align-queue-stats-response-shape.md`.

Triage skips `backlog/` subdirectories, so the item will not be re-picked
automatically. A human reviews it and either:

1. applies the denylisted change (`rust/fixtures/server/core_router.json`) directly,
2. re-scopes the item so it avoids the denylisted file and moves it back to
   `backlog/`, or
3. deletes it.
