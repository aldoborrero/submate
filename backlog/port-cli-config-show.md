# Port `submate config show` display formatting

**blocked-by:** none (submate-config env resolution already ported)

## what
Port the pure-data display transformation behind `submate config show` in
`submate/cli/commands/config.py`. This is the deterministic text layer the
broad `port-cli-commands` item does NOT faithfully capture: its falsifier
claims `submate config` "prints resolved config equal to
`config/defaults.resolved.json`" (raw JSON), but the real Python command
renders a Rich table of **flattened, human-formatted, title-cased** rows — not
JSON. Three composable functions to port exactly:

- `_format_value(value)` — leaf rendering rules:
  - list → `", ".join(...)`, or `"(none)"` when the list is empty
  - bool → `"Yes"` / `"No"`
  - empty-string or `None` → `"(not set)"`
  - else → `str(value)`
  - NOTE: bool must be checked before the empty/None branch, and Python's
    `value == ""` does NOT match `0`/`0.0` here because numeric leaves render
    via `str(value)`; preserve that ordering precisely.
- `_flatten_settings(value, prefix="")` — depth-first flatten of the nested
  `model_dump(mode="json")` dict into ordered `(dotted_name, display)` rows;
  enums already serialized to their `.value` strings by `mode="json"`, so the
  Rust side must flatten the same serde-JSON tree (insertion/field order
  preserved, matching Python dict order = struct field declaration order).
- display-name title-casing: for each dotted segment,
  `part.replace("_", " ").title()` then `".".join(...)` of the parts
  (e.g. `whisper.compute_type` → `Whisper.Compute Type`,
  `translation.openai_api_key` → `Translation.Openai Api Key`). Match Python
  `str.title()` semantics (capitalize first letter of each whitespace-run).

Emit the ordered `(setting, value)` rows as the unit under test (the Rich
`Table` chrome itself — borders/colors — is out of scope; the rows are the
contract). Reuse the config types from `submate-config`; do NOT re-resolve env
here, just walk a resolved `Config` serialized to serde-JSON.

## where
`rust/crates/submate-cli/src/config_show.rs` (new module; pure function
`config_show_rows(config_json: &serde_json::Value) -> Vec<(String, String)>`),
wired into the `config show` subcommand under `port-cli-commands`. Keep it
free of clap/IO so it is unit-testable without the rest of the CLI.

## why
`config show` is the user's primary way to verify resolved settings; the
flatten + format + title-case rules are exact-match pure-data output that must
not silently drift (e.g. `(not set)` vs empty, `Yes/No` vs `true/false`,
`(none)` for empty lists). Splitting it out unblocks a falsifiable test now,
without waiting on the whole CLI (whisper-pipeline, node-agent) that the
umbrella `port-cli-commands` is blocked behind.

## falsifies
`cargo test -p submate-cli config_show_rows` feeds the resolved default Config
(and an env-overridden Config) as serde-JSON to `config_show_rows` and asserts
the ordered `(setting, value)` rows equal a golden via `assert_json_eq`.

fixture (ALREADY LANDED on main, commit a73fffb — do NOT re-capture, do NOT
touch `rust/fixtures/`): `rust/fixtures/cli/config_show.defaults.rows.json` —
the row list from `submate config show` with no env set, an ordered list of
`[setting, value]` pairs.

fixture (ALREADY LANDED on main, commit a73fffb): `rust/fixtures/cli/config_show.overridden.rows.json`
— same dump with a representative env set exercising every `_format_value`
branch (bool `Yes`/`No`, empty list `(none)`, populated list joined, unset key
`(not set)`, plain string/number). The capture script
`rust/fixtures/capture/capture_cli_config.py` is also already committed.

The falsifier is therefore a PURE Rust port: read the existing goldens via the
`parity` harness, assert `config_show_rows` output equals them. No
`rust/fixtures/` write is needed, so there is no denylist bounce — this item is
directly dispatchable to an implementer.

---

**META note (round 2 unpark, 2026-06-12):** re-verified the gate is *phantom*,
not a human/credential gate. This was routed to `needs-human/` as a denylist
"capture-blocked" item, but its capture is **pure-data with no external
runtime** — `submate.cli.commands.config` imports cleanly in the nix devshell
(`nix develop --command python3 -c 'import submate.cli.commands.config'` succeeds). Per the documented
triage rule in `backlog/meta-contention.md` (pure-data captures → capture
pre-pass runs the capture, item lives in `backlog/`; only external-runtime
captures stay in `needs-human/`), this belongs in `backlog/`. Next round's
capture pre-pass should author `rust/fixtures/capture/capture_cli_config.py` and land the golden in a deliberate
capture commit before dispatch — do NOT re-park to `needs-human/`.

---

**META note (round 3 correction, 2026-06-12):** the round-3 re-park to
`needs-human/` (commit ea19970 lineage: ea19970 / 7605b49,
reason "denylisted capture script") **violated the triage rule already documented
in `backlog/meta-contention.md`**: pure-data captures (this item has no external
runtime) are owned by the *capture pre-pass*, which authors the capture script and
commits the golden BEFORE dispatch — they do NOT belong in `needs-human/` and must
NOT be handed to an implementer (who would then self-author the denylisted fixture
and bounce). Restored to `backlog/`.

**META note (post-merge round, 2026-06-12) — GATE RESOLVED, unparked:** the
capture pre-pass blocker is gone. Commit `a73fffb` ("fixtures(cli): salvage
stranded config-show + translate-filename goldens") landed BOTH goldens
(`config_show.defaults.rows.json`, `config_show.overridden.rows.json`, non-empty,
3.1 KB each) AND the capture script `capture_cli_config.py` on `main`. The sibling
`port-cli-translate-filename-logic` already merged this round on the same basis.
Verified this round: fixtures present and populated (cover `(none)`/`No`/title-case
branches), `rust/crates/submate-cli/src/config_show.rs` not yet ported. The item is
now a pure port reading existing goldens — no `rust/fixtures/` write, no denylist
bounce. Moved back to `backlog/` and dispatchable as normal.
