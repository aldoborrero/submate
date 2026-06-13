# meta: token-cost reporting tooling is missing from the repo

## observed (round 3 META, 2026-06-13)

The META supervisor prompt instructs running
`.claude/workflows/token-cost.sh --by-role` / `--notes` to produce the
per-role cost table, attach per-merge cost as `refs/notes/tokens`, and push
those notes. The script does not exist in the repo:

- `.claude/workflows/` on `origin/main` contains only `grind-base.js`.
- No `token-cost.sh` anywhere in the tree (`fd -t f 'token-cost'` -> empty).
- `.claude/workflows/tool-hints.md` (also referenced by the META prompt) is
  likewise absent.

Effect: the token-cost step is silently skipped every round. The per-role
trend is never committed, so WIDE (med >= 2x impl_med) and DRY (>= 3 runs,
< 0.5 filed/run) flags can't fire, and no `refs/notes/tokens` accumulate.
This disables the only mechanism that would tell us to split or retire a
specialist role.

## what's needed (human)

Decide and provide one of:

1. Check in `.claude/workflows/token-cost.sh` implementing `--by-role`
   (per-role token median/total table to stdout) and `--notes` (write
   per-merge cost into `refs/notes/tokens`). Source of truth for the
   numbers needs specifying — where does the grind record per-subagent
   token usage? (transcript sidecar? a JSON the orchestrator writes?)
2. Or amend the META prompt to drop the token-cost step until the tooling
   exists, so it stops appearing as an unmet instruction each round.

Until resolved, META cannot act on role WIDE/DRY signals.

## also note

`tool-hints.md` is referenced for output-size guidance but absent. Either
check it in or drop the reference. Lower priority than the cost script.
