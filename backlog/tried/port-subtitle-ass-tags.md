# tried: port-subtitle-ass-tags

**Outcome:** Abandoned — scope violation (denylist hit).

## What happened

The grind attempt modified a file outside the allowed scope for this item:

- `rust/fixtures/subtitle/tags_basic.ass`

This file is on the denylist, so the change could not be auto-applied and the
branch/worktree were discarded.

## Disposition

- Worktree `port-subtitle-ass-tags` removed.
- Branch `grind/port-subtitle-ass-tags` deleted.
- Item rerouted to `backlog/needs-human/port-subtitle-ass-tags.md`.

Triage skips `backlog/` subdirectories, so the item will not be re-picked
automatically. A human reviews it and either:

1. applies the denylisted change (`rust/fixtures/subtitle/tags_basic.ass`) directly,
2. re-scopes the item so it avoids the denylisted file and moves it back to
   `backlog/`, or
3. deletes it.
