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
- Item initially rerouted to `backlog/needs-human/`, then unparked back to
  `backlog/port-bazarr-language-name-lookup.md`.

The denylist hit was on a capture *script* (`rust/fixtures/capture/capture_bazarr_lang.py`),
not an external-runtime gate. Per the capture-prepass triage rule
(`meta-contention.md`, fifth pattern), a capture-blocked item whose capture is
pure-data with no external runtime belongs in `backlog/`, not `needs-human/`.
This item's body confirms it is a pure-data table + two-step lookup with no
whisper/runtime dependency, so it was unparked.

The correct fix is a **capture pre-pass**: author `capture_bazarr_lang.py` and
land its golden under `rust/fixtures/**` in a dedicated capture commit, then
re-dispatch the porter (whose scope excludes `rust/fixtures/**`) against the
now-present oracle.
