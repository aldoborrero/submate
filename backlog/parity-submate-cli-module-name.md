# Parity discoverability: submate-cli golden tests live in `mod tests`, invisible to `parity::`

**blocked-by:** none (test-module rename only; no src logic change, no fixture change)

## what
The two fixture-driven golden parity tests in `submate-cli` pass, but they are
declared inside `#[cfg(test)] mod tests`, not `mod parity`. The grind's canonical
parity falsifier command filters on the `parity::` path:

```
cargo test -p submate-cli parity::
```

Against the current tree this matches **0 tests** (`0 passed; 0 failed; 3 filtered out`).
The golden assertions only run under the bare `cargo test -p submate-cli`
(`config_show::tests::*`, `translate_paths::tests::translate_filename_cases`),
so the parity harness — and any reviewer running the documented command — is
blind to them. A future regression in `config_show_rows` or the translate
filename helpers would NOT be caught by the parity gate.

The affected, already-passing golden tests:

- `config_show::tests::config_show_rows_defaults` — diffs `config_show_rows`
  against `rust/fixtures/cli/config_show.defaults.rows.json`.
- `config_show::tests::config_show_rows_overridden` — vs
  `rust/fixtures/cli/config_show.overridden.rows.json`.
- `translate_paths::tests::translate_filename_cases` — table-drives every row of
  `rust/fixtures/cli/translate_filename_cases.json`.

All three already call `parity::{assert_json_eq, golden}` from the shared helper
crate, so the *content* is genuine parity — only the discovery path is wrong.

## where
- `rust/crates/submate-cli/src/config_show.rs` — `mod tests` (around the
  `#[cfg(test)]` block).
- `rust/crates/submate-cli/src/translate_paths.rs` — `mod tests`.

## fix
Follow the established precedent in `submate-jellyfin/src/lib.rs`, which keeps
plain unit tests in `mod tests` and fixture/golden parity in a dedicated
`mod parity` (and `submate-bazarr/src/lib.rs`, which uses `mod parity`). Either:

1. Rename the golden-bearing `mod tests` → `mod parity` in both cli files (these
   modules contain *only* golden assertions today, so a straight rename is
   safe), **or**
2. Split: leave non-golden unit tests under `mod tests` and move the three
   fixture-driven `#[test]` fns into a sibling `mod parity` in each file.

No `src` logic, public API, or `rust/fixtures/**` changes — test-module
attribute/name only.

## falsifies
After the change, the canonical command must discover and pass them:

```
cargo test -p submate-cli parity::
```

must report `3 passed; 0 failed; 0 filtered out` and include:

- `config_show::parity::config_show_rows_defaults`
- `config_show::parity::config_show_rows_overridden`
- `translate_paths::parity::translate_filename_cases`

(Bare `cargo test -p submate-cli` continues to pass — this is purely making the
existing green assertions reachable through the `parity::` gate.)

## why
The grind's whole parity contract hinges on `cargo test -p <crate> parity::`
being the single source of truth for "does this crate match the Python golden".
A crate whose golden tests are unreachable through that command reports a false
"no parity coverage" to the harness while silently carrying real coverage —
exactly the gap that lets a later edit regress `config show` formatting or a
translate filename rule without the gate going red.
