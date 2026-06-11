# Parity: submate-types enum `.value` strings vs golden

**kind:** parity-test-missing
**crate:** submate-types
**fixture:** `rust/fixtures/types/enum_values.json`
**relates:** port-types-enums (the port item builds the enums; this asserts parity)

## context
`rust/fixtures/types/enum_values.json` is a golden capture of every member of
the six `StrEnum`s in `submate/types.py` (`WhisperModel`,
`WhisperImplementation`, `Device`, `TranscriptionTask`, `LanguageNamingType`,
`TranslationBackend`) mapping `VARIANT -> .value`. Verified consistent with the
Python source on 2026-06-11 (all members present, all values exact).

No `parity::` test in `submate-types` loads this golden — `cargo test -p
submate-types parity::` currently runs **0 tests**. The enum `.value` strings
are load-bearing for config/serde parity (e.g. `"faster-whisper"`,
`"large-v3"`, `"iso_639_2_t"` contain chars that a naive derive would mangle to
`faster_whisper` / `large_v_3` / etc.), so this is the highest-leverage missing
parity test: every downstream crate depends on these strings matching Python.

## falsifies
`nix develop --command cargo test --manifest-path rust/Cargo.toml -p
submate-types parity::enum_values` exists and passes, asserting **exact**
(`parity::assert_json_eq`) equality between the golden and a serialized map of
each enum's `VARIANT -> Display`/`.to_string()` for all six enums and every
variant. The test must:

- include all 6 enums and every variant in `enum_values.json` (no subset),
- compare via the parity exact helper (byte-for-byte JSON), and
- fail if any variant string drifts (e.g. `large-v3` -> `large_v3`).

This is the implementer-blind reproduction of the Python `.value` contract for
the foundation crate.
