# tried: bug-parity-submate-paths-dot-relative-parent

**Outcome:** Abandoned — scope violation (denylist hit).

## What happened

The grind attempt modified a file outside the allowed scope for this item:

- `rust/fixtures/capture/capture_paths.py`

This file is on the denylist, so the change could not be auto-applied and the
branch/worktree were discarded.

## Disposition

- Worktree `bug-parity-submate-paths-dot-relative-parent` removed.
- Branch `grind/bug-parity-submate-paths-dot-relative-parent` deleted.
- Item rerouted to `backlog/needs-human/bug-parity-submate-paths-dot-relative-parent.md`.

Triage skips `backlog/` subdirectories, so the item will not be re-picked
automatically. A human reviews it and either:

1. applies the denylisted change (`rust/fixtures/capture/capture_paths.py`) directly,
2. re-scopes the item so it avoids the denylisted file and moves it back to
   `backlog/`, or
3. deletes it.
