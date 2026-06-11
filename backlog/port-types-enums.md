# Port types.py enums to submate-types

## what
Port the enums in `submate/types.py` (WhisperModel, WhisperImplementation, Device, TranscriptionTask, LanguageNamingType, TranslationBackend) to Rust in `submate-types`, using `strum` so each variant's `Display`/`FromStr` matches Python's `.value` string exactly.

## where
`rust/crates/submate-types/src/lib.rs`. Add `serde` + `strum` (workspace deps).

## why
Foundational — every other crate depends on these enums, and their string values must match Python for config/serde parity.

## falsifies
`nix develop --command cargo test --manifest-path rust/Cargo.toml -p submate-types parity::enum_values` passes exact-match against `rust/fixtures/types/enum_values.json` (every variant serializes to the Python string).
