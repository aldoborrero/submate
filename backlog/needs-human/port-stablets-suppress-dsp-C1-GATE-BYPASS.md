# GATE BYPASS: port-stablets-suppress-dsp-C1 merged WITH denylisted fixtures

**Status:** needs human review — denylist violation reached `origin/main`.

## What happened

This item was dispatched to the **abandon** path because its branch scope
included a `mergeDenylist` hit: `rust/fixtures/stablets/clipA/loudness.f32`
(`mergeDenylist` contains `/^rust\/fixtures\//`, see `.claude/grind.config.js`).

Concurrently, a merge path also ran for the same branch. The merge won the
race and was pushed to `origin/main`:

- merge commit `371e08e` ("Merge grind/port-stablets-suppress-dsp-C1: port non-VAD silence DSP")
- second parent `53815c3` (grind/port-stablets-suppress-dsp-C1 tip)
- denylisted files now live on `main`:
  - `rust/fixtures/stablets/clipA/loudness.f32`
  - `rust/fixtures/stablets/clipA/mask.f32`

The abandon agent was given stale instructions assuming the branch was still
unmerged. It did **not** fabricate a "tried/abandoned" record, because the work
was merged as done — the backlog item `backlog/port-stablets-suppress-dsp-C1.md`
was already removed by the merge, not abandoned.

## Why the gate let it through (suspected)

`mergeOne` in `.claude/workflows/grind-base.js` derives scope from a batched
scope-probe agent (`scopeMap`). The abandon decision and the merge decision both
fired for this branch — a race in the serialized merge queue vs. the abandon
fork. Net effect: a denylisted golden-fixture change landed on `main` as a side
effect of porting code, which the contract explicitly forbids
(grind.config.js: "Golden fixtures change only via a deliberate capture run ...
never as a side effect of porting code").

Note also `let bad = allBad[0]` only surfaces the FIRST denylist hit; both
`loudness.f32` and `mask.f32` were hits.

## Decision needed from a human

Pick one:

1. **Bless the fixtures** — if the goldens are correct, treat `371e08e` as a
   legitimate deliberate capture. Add `rust/fixtures/stablets/clipA/loudness.f32`
   and `mask.f32` to `rust/fixtures/README.md`, and close this out.
2. **Revert** `371e08e` on `main` (`git revert -m 1 371e08e`), re-scope the
   port item to exclude `rust/fixtures/`, capture the goldens via a proper
   capture item, then re-run the port.
3. **Re-capture only the fixtures** deliberately and keep the code.

This was left in `needs-human/` (triage skips subdirs) rather than recorded as a
clean abandonment, because the original abandon instructions no longer match
reality: the branch is merged and published, not abandoned. Acting on the stale
instructions (re-adding the backlog item to needs-human + writing a
"tried/abandoned" record) would have created a false ledger.
