# Port backlog

Work items for the submate Python→Rust port, consumed by `/grind` (see
`.claude/grind.config.js`). Triage ignores this README (`ls *.md | grep -v README`).

Each item is one-implementer / one-worktree-sized and follows:

```
# <title>

**blocked-by:** <comma-separated slugs>   (omitted if none)

## what       — the change
## where      — target crate + files
## why        — rationale
## falsifies  — the objective pass/fail check that proves it's done
```

The falsifier is almost always a named test, e.g. `cargo test -p <crate>
parity::<x>` passing against a golden fixture under `rust/fixtures/`. An
implementer is done iff that command is green under the nix-devshell `fastCheck`.

**Two falsifier kinds.** Pure-data ports use **parity** falsifiers (exact diff
vs a Python golden). The server↔node **coordination layer** (`submate-queue`
claim/lease, `submate-server` node API, `submate-node` agent) is a *new design*
(FileFlows/Unmanic topology — see `rust/docs/architecture.md`), not a port of
Python's single-box queue, so those items use **behavioral/integration**
falsifiers (atomic claim, lease reclaim, pull-loop against a mock server). The
business logic they carry (the 9 skip conditions, Bazarr output formatting)
keeps parity falsifiers.

## Prerequisite: golden fixtures

Most falsifiers diff against `rust/fixtures/`. Capture those once from the
Python tree before launching the grind — see `rust/fixtures/capture/README.md`.
Items whose fixture is media-dependent (transcription, media) need a sample
clip captured first.

## Order

Items carry `blocked-by` edges encoding the dependency order: foundational pure
crates (types → lang/config/paths) → leaf utils (subtitle) → the stable-ts slice
(model A → regroup B → suppress C → output D) → whisper → translate → queue →
server → cli/integrations. Triage's sibling-cluster guard naturally serializes
the `port-stablets-*` cluster, which is correct — that crate is highest-risk.

The T0 scaffold (`rust/` workspace + the `parity` crate) and the nix-devshell
toolchain are already done on the branch that introduced this backlog; the items
here start at T1.
