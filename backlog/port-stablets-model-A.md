# stable-ts (A): WhisperResult / Segment / WordTiming model

## what
Port the data model: `WordTiming`, `Segment`, `WhisperResult` structs with their fields, derived `start/end/text`, locking flags, and a `to_dict`-equivalent serde representation (3-decimal rounding to match Python).

## where
`rust/crates/stable-ts/src/model.rs`.

## why
Foundation of the stable-ts slice; B/C/D all operate on this model. Highest-risk crate — kept pure and golden-gated.

## falsifies
`cargo test -p stable-ts parity::model_roundtrip` parses `rust/fixtures/stablets/*/00_raw.json` and re-serializes byte-identically.
