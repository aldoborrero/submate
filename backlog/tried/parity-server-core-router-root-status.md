# tried: parity-server-core-router-root-status

## outcome
Abandoned — scope violation (denylist hit).

## what happened
The grind branch `grind/parity-server-core-router-root-status` modified a file
outside the item's allowed scope:

- `rust/fixtures/capture/README.md`

This path is on the denylist. Everything under `rust/fixtures/` (including the
`capture/` authoring docs and scripts) is golden parity data and must be
authored by a human or a deliberate capture pre-pass, never by the automated
port item that the goldens falsify — a porter editing its own oracle defeats
the parity check. Because the porter touched the denylisted file, the branch
could not be auto-applied and was rejected.

## actions taken
- Removed worktree `parity-server-core-router-root-status` and deleted branch
  `grind/parity-server-core-router-root-status`.
- Restored `backlog/parity-server-core-router-root-status.md` from `origin/main`
  and rerouted it to `backlog/needs-human/parity-server-core-router-root-status.md`.
  Triage skips `backlog/` subdirectories, so the item will not be auto-picked
  again until a human acts on it.

## next steps (human)
A human reviews `backlog/needs-human/parity-server-core-router-root-status.md`
and either:
1. applies the denylisted change directly (authors the
   `rust/fixtures/capture/README.md` change, plus any capture run needed to
   land the `rust/fixtures/server/core_router.json` golden), then re-runs the
   item, or
2. re-scopes the item to exclude the denylisted file and moves it back to
   `backlog/` for automated pickup, or
3. deletes the item if it is no longer wanted.
