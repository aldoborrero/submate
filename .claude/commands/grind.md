---
description: Multi-agent Python→Rust port of submate — implementers + rotating port specialist (parity/scout/aligner/simplifier/curator/documenter), cargo-gated
---

Launch the submate-rs port grind. Runs until dry-streak or `.grind-stop`.

| Agent | Role |
|---|---|
| **parity** | runs ported crates' `parity::*` tests; files `bug-parity-*` (with the exact Python-vs-Rust diff) or `parity-*` (missing test) |
| **porter-scout** | finds un-ported Python surface; files decomposed, dependency-ordered `port-*` items with parity falsifiers |
| **aligner** | config keys (`SUBMATE__`/`__`), enum `.value` strings, route signatures must match Python exactly |
| **simplifier** | cruft reduction — net-negative lines, parity tests stay green |
| **curator** | Cargo dep set + `cargo audit` CVE scan |
| **documenter** | `rust/README.md` port map + `rust/fixtures/README.md` |
| **architect** (every 6 rounds) | crate boundaries, the pure-data/I/O seam, the async model |
| **Implementer ×3** | one `backlog/` item each in `../submate-rs-grind/<slug>` |
| **Merge** | serialized; gate = `cargo test + clippy -D warnings` via the nix devshell |

## Launch

Workflow is split: CONFIG in `.claude/grind.config.js`, generic loop in
`.claude/workflows/grind-base.js`.

Prerequisites: this branch must be on `origin/main` so implementers can branch
from it (the `rust/` scaffold, `backlog/`, and grind config all need to be
landed first). The nix devshell provides cargo + clippy — `fastCheck` enters it
via `nix develop --command`.

1. Check `ls ../submate-rs-grind/_base/.git 2>/dev/null` — if it exists, it may
   be stale from a killed session (the workflow will sync it). Only STOP if you
   see another /grind Workflow task actively running for this repo.
2. `rm -f .grind-stop`
3. Read both files, then `Workflow({script: project + "\n" + base, args: ${ARGUMENTS:-{}}})`.

Args: `/grind` · `/grind {rounds:3}` · `{rounds:N, implementers:M, dryLimit:K}`.

Stop with `touch .grind-stop`.
