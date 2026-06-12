# Parity discoverability: submate-subtitle discovery goldens are top-level in `tests/parity.rs`, invisible to `parity::`

**blocked-by:** none (test-module wrap only; no src logic change, no fixture change)

## what
The three fixture-driven golden parity tests in `submate-subtitle/tests/parity.rs`
pass, but they are declared as **top-level** integration-test functions. Inside
an integration-test binary the test path is just the bare function name
(`discovery_fs`, `lrc_paths`, `internal_probe`) — there is no `parity::`
component. The grind's canonical parity falsifier filters on that path:

```
cargo test -p submate-subtitle parity::
```

Against the current tree the `tests/parity.rs` binary reports
`0 passed; 0 failed; 3 filtered out` for that filter (verified). The discovery
golden assertions only run under a bare, unfiltered `cargo test -p
submate-subtitle`, so the parity harness — and any reviewer running the
documented command — is blind to them. A regression in
`get_external_subtitle_paths`, `parse_subtitle_language`, or `get_lrc_path`
would NOT be caught by the parity gate.

The affected, already-passing golden tests (all in
`rust/crates/submate-subtitle/tests/parity.rs`):

- `discovery_fs` — set-equality on `get_external_subtitle_paths` and the exact
  five-field language tuple from `parse_subtitle_language`, table-driven over
  every `discovery` case in `rust/fixtures/subtitle/discovery_cases.json`.
- `lrc_paths` — diffs `get_lrc_path` against the `lrc` map of the same golden.
- `internal_probe` — exact `get_internal_subtitle_languages` vs the
  capture-first `subtitle/clipS.subs.json` golden (self-skips when `ffprobe` or
  the denylisted clip fixtures are absent).

All three already call `parity::{golden, fixture_path}` from the shared helper
crate, so the *content* is genuine parity — only the discovery path is wrong.

This is the identical gap that `backlog/parity-submate-cli-module-name.md`
flagged for the cli golden tests (since fixed: `config_show.rs` and
`translate_paths.rs` now use `mod parity`). The in-source cue round-trip tests
in this same crate already do it right — `cue::parity::srt_roundtrip` etc. are
discovered and pass under `parity::`. Only the discovery integration binary was
left exposed.

## where
- `rust/crates/submate-subtitle/tests/parity.rs` — the three `#[test]` fns
  (`discovery_fs`, `lrc_paths`, `internal_probe`) sit at module top level
  alongside their helpers (`lang_tuple`, `TempDir`, `file_name`,
  `ffprobe_on_path`).

## fix
Wrap the three `#[test]` fns in a `mod parity { use super::*; ... }` so their
paths become `parity::discovery_fs`, `parity::lrc_paths`,
`parity::internal_probe`. Leave the shared helpers at file top level (the
module reaches them via `use super::*;`), matching the split-style option
already used elsewhere. No `src` logic, public API, or `rust/fixtures/**`
change — test-module wrap only.

## falsifies
After the change, the canonical command must discover and pass them:

```
cargo test -p submate-subtitle --test parity parity::
```

must report `3 passed; 0 failed; 0 filtered out` and include:

- `parity::discovery_fs`
- `parity::lrc_paths`
- `parity::internal_probe`

(Bare `cargo test -p submate-subtitle` continues to pass — this is purely making
the existing green assertions reachable through the `parity::` gate. The
`internal_probe` test continues to self-skip when `ffprobe` / the clip fixtures
are unavailable.)

## why
The `parity::` filter is the single command the grind, reviewers, and any future
regression check run to prove a layer matches the Python golden. A golden test
that exists but is silently filtered out is a false sense of coverage: the
subtitle discovery layer (external-file globbing, filename-language parsing,
.lrc path derivation) would appear "untested by parity" and could regress
undetected. Closing this makes the on-disk-discovery contract actually gated.
