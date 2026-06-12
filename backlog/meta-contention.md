# meta-contention: fixture-missing items churn through denylist abandonment

## pattern (observed round 3, 2026-06-12)

All 7 items in `backlog/tried/` were abandoned for the **same** reason:
`scope violation (denylist hit)` where the denylisted path is a
`rust/fixtures/**` golden the porter authored itself. This round, 3/3
dispatched items abandoned this way (0/3 merged).

The mechanism is structural, not per-item:

1. A port item's falsifier asserts against a golden fixture
   (`enum_values.json`, `path_cases.json`, `ops.json`, `stablets/*`).
2. That golden does not exist yet (or lacks the item's case).
3. The porter, needing a green test, authors the missing fixture.
4. `rust/fixtures/**` is merge-denylisted (goldens must be human/capture
   authored, never by the item they falsify — self-authored goldens defeat
   the parity check). The branch is rejected and the item parked.

`port-stablets-model-A` has cycled this 4 times — a chronic deferral driven
entirely by this pattern.

## proposed triage rule

Before dispatching a port item to an implementer, check whether its falsifier
fixture exists and covers the item's case:

- If the fixture exists and covers the case -> dispatch normally.
- If the fixture is **missing or under-covers** the case AND can be produced
  from the Python spec with no external runtime (pure-data captures:
  `capture_enums.py`, `capture_paths.py`, `capture_config.py`,
  `capture_lang.py`, `capture_translate.py`) -> **META/capture pre-pass runs
  the capture and commits the golden first**, then the item is dispatchable.
  (Done this round for `port-queue-models-enums`: META extended
  `capture_enums.py` + regenerated `enum_values.json`, moved item back to
  `backlog/`.)
- If the fixture requires an **external runtime** (Whisper model, audio clip,
  live server) -> route to `backlog/needs-human/` with the exact capture
  command, NOT to an implementer. These are genuine gates
  (`port-stablets-model-A/B1/C1`, `port-server-app-ops`).

## why this beats per-item annotation

Annotating each item "do not touch fixtures" does not help — the porter has no
green path without the golden. The fixture must materialize *before* dispatch,
either by capture pre-pass (cheap, pure-data) or by a human (runtime-gated).
The triage gate, not the porter, owns fixture existence.

## quick audit command

```sh
# items whose falsifier names a fixture that is absent from the tree
for f in backlog/*.md; do
  rg -o 'rust/fixtures/[^ `)]+' "$f" | while read -r fx; do
    [ -e "$fx" ] || echo "$f -> MISSING $fx"
  done
done
```

## second pattern (observed round 1 META, 2026-06-12): stranded merge + crate-root contention

The B1 regroup port (`port-stablets-regroup-parse-B1`) completed and merged
into `rust-port-scaffold` (436e5aa) but **never reached origin/main**: main
advanced 7 commits past that branch (including `suppress-dsp-C1`), leaving
both the `regroup.rs` work and its backlog item stranded. META salvaged it
this round onto `meta/salvage-b1-regroup`, but the merge hit conflicts.

The conflict locus was **not** a fixture — it was the stable-ts crate root.
Every stable-ts sub-port (model-A, suppress-C1, regroup-B1, splits-B2) edits
the same two lines-of-contention:

- `rust/crates/stable-ts/src/lib.rs` — the `pub mod` / `pub use` list.
- `rust/crates/stable-ts/tests/parity.rs` — the shared `use` import line and
  the appended `#[test]` block.

These are purely additive edits, so they conflict on every concurrent pair
even though the changes are independent. `lib.rs` has been touched in 9
commits across branches.

### proposed triage rule

Serialize stable-ts sub-ports through the merge queue (do not dispatch two
stable-ts items in the same wave), OR refactor the crate root so each
sub-module self-registers without editing a shared list — e.g. move the
`pub mod`/`pub use` lines into per-submodule files and have `parity.rs`
pull tests via `include!`-per-module rather than one append-only file. The
former is cheaper; the latter removes the contention permanently.

## third pattern (observed round 1 META, 2026-06-12): monolithic submate-server/lib.rs

`rust/crates/submate-server/src/lib.rs` is a single 1680-line file holding the
entire server crate (routes, handlers, state, error mapping). It is the hottest
file in the recent merge window — touched in 3 of the last 5 first-parent
merges (`188e023` embedded-node, `ecf3549` http-error-body-detail-key,
`4fa64bb` audio-transfer). All three merged cleanly **this** round, so this is a
leading indicator, not yet a realized conflict.

The risk mirrors the stable-ts crate root: as more server sub-ports land
(bazarr handler, jellyfin webhook, queue-status routes), concurrent waves will
increasingly append routes/handlers to the same file and the same router-builder
function, producing additive conflicts that are independent in intent.

### proposed triage rule

Before the next wave that dispatches 2+ server-handler items, split
`submate-server/src/lib.rs` into per-area modules
(`handlers/bazarr.rs`, `handlers/jellyfin.rs`, `handlers/core.rs`, `state.rs`,
`error.rs`) mirroring the Python `submate/server/handlers/` layout, with `lib.rs`
reduced to module declarations + router assembly. This pre-empts the same
append-only contention before it forces serialization. Tracked separately as a
port-refactor item if/when a server wave is queued; noted here so the trend is
in git.
