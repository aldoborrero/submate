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

requires fixture: `rust/fixtures/cli/config_show.defaults.rows.json` (capture
first) — the row list from `submate config show` with no env set; capture by
running the Python command over `Config()` and dumping
`_flatten_settings(cfg.model_dump(mode="json"))` after the title-case map, as
an ordered list of `[setting, value]` pairs.

requires fixture: `rust/fixtures/cli/config_show.overridden.rows.json`
(capture first) — same dump but with a representative env set exercising every
`_format_value` branch: a bool (e.g. a `Yes`/`No` flag), an empty list
(`(none)`), a populated list (joined), an unset key (`(not set)`), and a
plain string/number. Reuse the env from `rust/fixtures/config/validators.env`
if it already exercises these branches; otherwise capture a new `.env`
alongside.

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
