# tried: port-stablets-regroup-parse-B1

**Outcome:** Parked — needs-design-decision (crate-root merge contention).

## Why this is parked, not abandoned

The port itself is **complete** — it was implemented and merged into a port
branch at least twice (`3aeeb26`, `7c0d767`, both "Port stable-ts regroup DSL
parser"), and its falsifier fixture `rust/fixtures/stablets/regroup_parse.json`
exists in the tree. The blocker is **not** scope, fixtures, or the stale
`blocked-by: port-stablets-model-A` (the `model.rs` module has since landed,
`602bcb4`). The blocker is structural merge contention.

## The churn (deleted/restored 3×)

`port-stablets-regroup-parse-B1` has cycled:
needs-human (scope) → backlog (fixtures captured) → tried-dup cleared →
completed twice on port branches that **never reached origin/main**. Each
completion stranded because main advanced past the port branch and the merge
conflicts on the stable-ts crate root.

## The design decision required

Documented in `backlog/meta-contention.md` ("second pattern"): every stable-ts
sub-port (model-A, suppress-C1, regroup-B1, splits-B2) makes purely-additive
edits to the same two lines-of-contention —

- `rust/crates/stable-ts/src/lib.rs` (the `pub mod` / `pub use` list)
- `rust/crates/stable-ts/tests/parity.rs` (shared `use` + appended `#[test]`)

so they conflict on every concurrent pair despite being independent. `lib.rs`
has been touched in 9 commits across branches.

A human must pick one:

1. **Cheap:** serialize stable-ts sub-ports through the merge queue (never
   dispatch two stable-ts items in the same wave) and re-land the existing
   regroup work as the next single stable-ts merge.
2. **Permanent:** refactor the crate root so each sub-module self-registers
   without editing a shared list (e.g. per-submodule registration; `parity.rs`
   pulls tests via `include!`-per-module instead of one append-only file),
   removing the contention class entirely. Then re-dispatch.

## Disposition

- Item moved to `backlog/tried/` so triage stops re-picking it (triage skips
  `backlog/` subdirectories). Returns to `backlog/` once the crate-root design
  decision above is applied.
- Orphan worktree `port-stablets-regroup-parse-B1` still holds the WIP port
  (`rust/crates/stable-ts/src/regroup.rs`, ~382 lines) — salvage it when
  re-landing rather than re-porting from scratch.
