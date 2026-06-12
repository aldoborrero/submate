# meta: merge queue vs. abandon fork can both fire for one branch

## symptom
`port-stablets-suppress-dsp-C1` was simultaneously (a) dispatched to the abandon
path for a `mergeDenylist` hit (`rust/fixtures/stablets/clipA/loudness.f32`,
`mask.f32` — both match `/^rust\/fixtures\//`) and (b) merged to `origin/main`
(merge `371e08e`). The merge won the race, so denylisted golden fixtures with no
Python capture provenance landed on `main` and the parity falsifier passed
vacuously (Rust output asserted against Rust-authored "goldens").

META reverted the merge in `7d3abd2` and re-scoped the backlog item to require a
deliberate Python capture first.

## root cause (suspected, from grind-base.js)
`mergeOne` and the abandon decision both derive scope from the batched scope
probe (`scopeMap`) and are not mutually exclusive in the serialized queue — a
branch flagged for abandon can still reach `mergeOne` before the abandon fork
removes it. Two bugs compound it:

1. The denylist check surfaces only the FIRST hit (`let bad = allBad[0]`), hiding
   that both `loudness.f32` and `mask.f32` violated.
2. A denylist hit routes to *abandon* asynchronously rather than hard-failing the
   *merge* for that branch, so the merge path has no denylist guard of its own.

## proposed fix (needs human / harness edit — grind-base.js is mergeDenylist'd)
- In `mergeOne`, re-check `mergeDenylist` against the branch's own diff
  (`git diff --name-only origin/main...<branch>`) immediately before committing
  the merge; if any file matches, hard-skip and route to abandon. Make the merge
  path the authority, not a parallel async decision.
- Report ALL denylist hits, not just `allBad[0]`.
- Ensure abandon and merge are mutually exclusive per branch within one round
  (claim the branch before dispatch).

## contention note
This is the only same-file class to reach `main` improperly this round; normal
merges (`git log --merges`) show clean `ort` merges with no recurring code-file
conflicts. The contention here is governance (fixtures denylist), not source
conflicts.
