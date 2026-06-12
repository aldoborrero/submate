# meta: capture pre-pass stage is prescribed but not enforced

## symptom (round 3, 2026-06-12)

Two pure-data CLI items have now ping-ponged twice between `backlog/` and
`backlog/needs-human/`:

- `port-cli-config-show` (round 2 unparked -> round 3 re-parked: cf43bbb,
  12e22b7, ea19970)
- `port-cli-translate-filename-logic` (round 2 unparked -> round 3 re-parked:
  5e90978, 95994de, 7605b49)

Each re-park reason is "the grind attempt touched a denylisted capture script
(`rust/fixtures/capture/capture_cli_*.py`)". Each unpark reason is "the gate is
phantom — the module imports cleanly, the capture is pure-data". Both are
correct, which is exactly why the item cycles: an implementer cannot author the
denylisted golden, and nothing else does it either.

## root cause

`backlog/meta-contention.md` already prescribes the fix (pure-data captures ->
"META/capture pre-pass runs the capture and commits the golden first, then the
item is dispatchable"). But that pre-pass is **not an enforced workflow stage** —
it ran ad hoc for `port-queue-models-enums` and `capture_enums.py` in an earlier
round, then did not run for these two, so they fell back to the implementer path
and bounced. A documented rule with no owning stage is not a rule.

Distinguishing data point: the CLI captures need **new** scripts that do not yet
exist — `rust/fixtures/capture/capture_cli_config.py` and
`capture_cli_translate.py`, plus a new `rust/fixtures/cli/` golden dir. The
existing `capture_config.py` / `capture_translate.py` cover the already-ported
`submate-config` / `submate-paths` crates, not the CLI display/filename helpers.

## what a human needs to decide / wire

1. Make the capture pre-pass a real stage that runs BEFORE dispatch each round:
   scan `backlog/*.md` for `requires fixture:` / `rust/fixtures/` references whose
   target is absent, and for pure-data ones, author the capture script (following
   `rust/fixtures/capture/_common.py`) and commit the golden in a deliberate
   capture commit. Only external-runtime captures (Whisper model, audio, live
   server) stay in `needs-human/`.
2. Until that stage exists, treat these two items as blocked on it — they are
   restored to `backlog/` with the exact capture spec in their bodies (the
   `_format_value` / `_flatten_settings` / title-case rules for config; the
   `is_subtitle_file` / `detect_source_language` / output-path rules for
   translate). A human (or META) can author the two capture scripts directly from
   those specs; the Python source is `submate/cli/commands/{config,translate}.py`
   and imports cleanly in the nix devshell.

## verification done this round

`nix develop --command python3 -c 'import submate.cli.commands.config'` -> OK
`nix develop --command python3 -c 'import submate.cli.commands.translate'` -> OK

Phantom gate reconfirmed; items belong in `backlog/`, blocked on the pre-pass.

## update (round 4, 2026-06-12): pre-pass STILL did not run; port landed anyway

`port-cli-translate-filename-logic` was dispatched a 4th time with a plan that
asserted "capture pre-pass landed `rust/fixtures/cli/translate_filename_cases.json`
(non-empty) this round". It did NOT: there is still no `rust/fixtures/cli/` dir,
no `translate_filename_cases.json`, and no `capture_cli_translate.py`. This is
the 3rd consecutive round the prescribed pre-pass failed to materialize the
golden — confirming the stage is missing/broken, not merely skipped once.

To stop the ping-pong, the implementer this round landed the **pure port without
the fixture**: `rust/crates/submate-cli/src/translate_paths.rs` (the three
byte-for-byte helpers) is committed and covered by 9 inline unit tests asserting
every mandated golden row directly from the Python spec (no `rust/fixtures/`
touched, so no denylist bounce). Full `cargo test --workspace` + `clippy
--all-targets -D warnings` green. The item is left in `backlog/` with a
remaining-work note: once the pre-pass lands the golden, swap the inline tests
for a `parity::assert_json_eq` loop over `rust/fixtures/cli/translate_filename_cases.json`.

Net: the *code* is no longer blocked on the pre-pass; only the
*fixture-driven falsifier* is. The human/META action item in this file is now
strictly to (a) wire the pre-pass stage, and (b) author
`capture_cli_translate.py` so the golden exists — at which point the swap above
is a ~10-line dev-dep + test change.

## RESOLVED (post-merge round, 2026-06-12): goldens + capture scripts landed

Commit `a73fffb` ("fixtures(cli): salvage stranded config-show + translate-filename
goldens") materialized the missing artifacts on `main`:
`rust/fixtures/cli/config_show.{defaults,overridden}.rows.json` (non-empty) and
`rust/fixtures/capture/capture_cli_config.py`. The ping-pong root cause (golden
absent, implementer forced to author a denylisted fixture) no longer applies to
either CLI item:

- `port-cli-translate-filename-logic`: MERGED this round (cd9eb5f). The remaining
  swap (inline tests -> `parity::assert_json_eq` over the now-present golden) is
  ordinary follow-up work, not a gate.
- `port-cli-config-show`: unparked from `needs-human/` back to `backlog/` this
  round; the goldens it needs exist, so it is a pure port and dispatchable.

The structural action item — make the capture pre-pass an *enforced* stage so a
future `requires fixture:` item does not bounce again — stays open. But no item is
currently blocked on it; this file is informational until the next golden-absent
item appears.
