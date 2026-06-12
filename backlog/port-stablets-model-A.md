# stable-ts (A): WhisperResult / Segment / WordTiming model

## what
Port the data model: `WordTiming`, `Segment`, `WhisperResult` structs with their fields, derived `start/end/text`, locking flags, and a `to_dict`-equivalent serde representation (3-decimal rounding to match Python).

## where
`rust/crates/stable-ts/src/model.rs`.

## why
Foundation of the stable-ts slice; B/C/D all operate on this model. Highest-risk crate — kept pure and golden-gated.

## falsifies
`cargo test -p stable-ts parity::model_roundtrip` parses `rust/fixtures/stablets/clipA/00_raw.json` and re-serializes byte-identically.

## fixture status (META 2026-06-12)
Golden present and dispatchable. `rust/fixtures/stablets/clipA/00_raw.json`
(11.7KB of real `stable_whisper` output) plus the source clip
`rust/fixtures/clips/clipA.wav` were captured and committed this round in
`86a2fb1`. The prior round's "needs-human (Whisper runtime)" gate referenced a
tree where the golden was still absent; that capture has since landed, so the
external-runtime step is already done. No fixture authoring is required by the
porter — the item is a pure struct + serde port against an existing golden.
Keep `rust/fixtures/**` out of the branch (it is merge-denylisted); only
`rust/crates/stable-ts/src/model.rs` should change.
