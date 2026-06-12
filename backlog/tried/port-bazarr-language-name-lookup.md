# tried: port-bazarr-language-name-lookup

**Outcome:** Abandoned — scope violation (denylist hit).

## What happened

The grind attempt modified a file outside the allowed scope for this item:

- `rust/fixtures/capture/capture_bazarr_lang.py`

This file is on the denylist, so the change could not be auto-applied and the
branch/worktree were discarded.

## Disposition

- Worktree `port-bazarr-language-name-lookup` removed.
- Branch `grind/port-bazarr-language-name-lookup` deleted.
- Item rerouted to `backlog/needs-human/port-bazarr-language-name-lookup.md`.

Triage skips `backlog/` subdirectories, so the item will not be re-picked
automatically. A human reviews it and either:

1. applies the denylisted change (`rust/fixtures/capture/capture_bazarr_lang.py`) directly,
2. re-scopes the item so it avoids the denylisted file and moves it back to
   `backlog/`, or
3. deletes it.
