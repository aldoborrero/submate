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

## fourth pattern (observed parity round, 2026-06-12): rust/Cargo.lock merge conflicts

The `grind/port-cli-translate-filename-logic` merge (`a7c1de7`) conflicted on
`rust/Cargo.lock` — the only conflict in the last 20 first-parent merges, so a
leading indicator rather than a chronic locus yet. Cause is structural and
purely additive: any two concurrent ports that add or bump a crate dependency
both rewrite overlapping `[[package]]` blocks in the single lockfile, which
three-way merge cannot reconcile even though the intents are independent. As the
remaining backlog (translation backends over reqwest, server handlers over axum,
queue services) keeps pulling new crates, concurrent waves will hit this more
often.

### proposed triage rule

`Cargo.lock` conflicts are mechanical, not semantic — never hand-resolve the
block markers. Resolve by taking either side then running `cargo update
--workspace --offline` (or a plain `cargo build`) to regenerate a consistent
lockfile, which the merge-queue step can do automatically. Cheaper still: when a
wave dispatches 2+ items that each add dependencies, have the merge queue
`git checkout --theirs rust/Cargo.lock && cargo build` as the standard
conflict-resolution path for that one file rather than treating it as a real
conflict. Noted here so the trend is in git.

## fifth pattern (META round 2, 2026-06-12): capture-prepass / denylist ordering loop

Three items this round (`port-bazarr-pcm-wav-wrap`, `port-subtitle-discovery-fs`,
`parity-server-core-router-root-status`) were abandoned as "denylist scope
violations" because the porter touched `rust/fixtures/capture/**` (a capture
script or its README). All three then got rerouted to `needs-human/`. But the
capture-script authoring those items require is the **capture pre-pass's job**,
not a human/credential gate — the items' own bodies say so. `port-bazarr-pcm-wav-wrap`
proves the happy path exists: its capture script + goldens were authored in a
deliberate capture commit and the port diffed against them, landing cleanly
(merged this round, parity tests 4/4). The other two have no external runtime
(`port-subtitle-discovery-fs` is a temp-dir/filename layout; the core-router
module imports cleanly: `import OK`), yet they keep cycling
`backlog → tried/needs-human → backlog` — a chronic re-park.

### root cause

The denylist correctly stops a *porter* from editing its own oracle, but no
**capture pre-pass runs before dispatch** to author the golden first. So the
porter hits the capture file, trips the denylist, and the item is abandoned
instead of being handed to the pre-pass. The reroute target (`needs-human/`) is
also wrong: it implies a human/credential gate that does not exist.

### proposed triage rule

Before dispatching any item whose `falsifies` block says "requires fixture …
(capture first)" and whose capture lives under `rust/fixtures/capture/`, the
merge/dispatch step must run a **capture pre-pass**: author the
`capture/*.py`, land the golden under `rust/fixtures/**` in a dedicated capture
commit, THEN dispatch the porter against the now-present oracle (porter scope
excludes `rust/fixtures/**`, which is fine — the golden already exists). A
capture-blocked item belongs in `backlog/`, never `needs-human/`, unless its
capture genuinely needs an external runtime (GPU/credential/network). If an
item is re-parked a 3rd time without the pre-pass running, treat it as a
harness ordering bug, not an item defect.

## sixth pattern (META parity round, 2026-06-13): submate-cli/src/main.rs command-tree hotspot

`rust/crates/submate-cli/src/main.rs` (907 lines) is the most frequently
touched file in the recent merge window — 8 of the last 12 first-parent merges
and 3 of the last 5 (`46ee11c` cli-result-summary, `b5e09ba` cli-model-flag,
`466b26d` cli-transcribe-collect). The crate already extracts per-command logic
into sibling files (`config_show.rs`, `transcribe_collect.rs`,
`translate_paths.rs`), but `main.rs` still owns the clap command tree, the arg
structs, and the dispatch `match`, so every CLI port appends a subcommand
variant + a dispatch arm + arg fields to the same three regions. All these
merges landed cleanly (additive edits to distinct subcommands), so this is a
realized-but-not-yet-conflicting hotspot, same shape as the stable-ts crate root
(pattern 2) and submate-server/lib.rs (pattern 3).

Contention is winding down here: only one CLI port (`port-cli-progress`)
remains active, so a structural split is likely not worth it now. Recorded so
the trend is in git: if a future wave dispatches 2+ CLI items concurrently, move
the clap command/arg definitions next to each subcommand's module (each
subcommand file exposes its own `Args` struct + a `fn run`) and reduce `main.rs`
to subcommand registration + dispatch, mirroring the per-command file layout
that already exists. Until then, serialize CLI items through the merge queue
rather than dispatching them in the same wave.
