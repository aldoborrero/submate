# tried: port-stablets-suppress-dsp-C1

**Scope violation (denylist hit):** `rust/fixtures/stablets/clipA/loudness.f32`
(also `rust/fixtures/stablets/clipA/mask.f32`) — matches `mergeDenylist`
`/^rust\/fixtures\//`. The branch was dispatched to the abandon path for this
reason.

**Outcome — NOT a clean abandonment:** a concurrent merge for the same branch
won the race and was pushed to `origin/main` as merge commit `371e08e`
(second parent `53815c3`), carrying the denylisted golden fixtures onto `main`.
The backlog item `backlog/port-stablets-suppress-dsp-C1.md` was therefore
removed by that merge (as "done"), not by abandonment.

The abandon agent deleted the worktree + `grind/` branch (already preserved in
the merge commit) but did **not** re-route the original item back to
`needs-human/` as a fresh port, because the work is merged and published —
doing so would have created a contradictory ledger.

Instead, the gate-bypass is recorded for human resolution at:
`backlog/needs-human/port-stablets-suppress-dsp-C1-GATE-BYPASS.md`
(revert vs. bless-the-fixtures vs. re-capture).
