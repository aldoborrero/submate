# tried: port-translate-ass-tag-validate

**Outcome:** Abandoned — scope violation (denylist hit).

## What happened

The grind attempt modified a file outside the allowed scope for this item:

- `rust/fixtures/capture/capture_translate_ass.py`

This file is on the denylist, so the change could not be auto-applied and the
branch/worktree were discarded.

## Disposition

- Worktree `port-translate-ass-tag-validate` removed.
- Branch `grind/port-translate-ass-tag-validate` deleted.
- Item rerouted to `backlog/needs-human/port-translate-ass-tag-validate.md`.

Triage skips `backlog/` subdirectories, so the item will not be re-picked
automatically. A human reviews it and either:

1. applies the denylisted change (`rust/fixtures/capture/capture_translate_ass.py`) directly,
2. re-scopes the item so it avoids the denylisted file and moves it back to
   `backlog/`, or
3. deletes it.
