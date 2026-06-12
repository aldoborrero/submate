# tried: port-cli-config-show

**Outcome:** Abandoned — scope violation (denylist hit).

## What happened

The grind attempt modified a file outside the allowed scope for this item:

- `rust/fixtures/cli/config_show.defaults.rows.json`

This path is on the denylist, so the change could not be auto-applied and the
branch/worktree were discarded. Fixtures under `rust/fixtures/` are golden
parity data: they are the oracle the Rust port is diffed against, so an
automated porter editing them would let the implementation define its own
expected output.

## Disposition

- Worktree `port-cli-config-show` removed.
- Branch `grind/port-cli-config-show` deleted.
- Item rerouted to `backlog/needs-human/port-cli-config-show.md`.

Triage skips `backlog/` subdirectories, so the item will not be re-picked
automatically. A human reviews it and either:

1. applies the denylisted change (`rust/fixtures/cli/config_show.defaults.rows.json`)
   directly, then re-runs the item,
2. re-scopes the item so it avoids the denylisted fixture and moves it back to
   `backlog/` for automated pickup, or
3. deletes it if it is no longer wanted.
