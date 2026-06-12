# stable-ts (A): WhisperResult / Segment / WordTiming model

## what
Port the data model: `WordTiming`, `Segment`, `WhisperResult` structs with their fields, derived `start/end/text`, locking flags, and a `to_dict`-equivalent serde representation (3-decimal rounding to match Python).

## where
`rust/crates/stable-ts/src/model.rs`.

## why
Foundation of the stable-ts slice; B/C/D all operate on this model. Highest-risk crate — kept pure and golden-gated.

## falsifies
`cargo test -p stable-ts parity::model_roundtrip` parses `rust/fixtures/stablets/*/00_raw.json` and re-serializes byte-identically.

## why blocked (needs-human, re-verified 2026-06-12 META)
The falsifier depends on fixtures under `rust/fixtures/stablets/*/00_raw.json`,
which do not exist yet. The capture script DOES exist
(`rust/fixtures/capture/capture_stablets.py`, not the `_model.py` name this
item's earlier note assumed) — but it is a real external-runtime boundary:
`python capture_stablets.py /path/to/clip.wav` loads a Whisper model via
`stable_whisper.load_faster_whisper` and downloads weights on first run, so it
needs a human/CI with an audio clip + model runtime. That gate is real, not a
false credential gate. `rust/fixtures/stablets/` is absent and `model.rs` does
not exist. Prior attempt `grind/port-stablets-model-A` was rejected for the
same fixture-absence (see `backlog/tried/port-stablets-model-A.md`). Touched 4
times across rounds — a chronic deferral; prefer re-scope option 2 below over
re-parking unchanged.

## human action (pick one)
1. Author `rust/fixtures/capture/capture_stablets_model.py` and run it against a
   real Whisper/stable-ts output to commit `rust/fixtures/stablets/<case>/00_raw.json`
   (external-boundary capture; must be done by a human/CI with the runtime), then
   move this item back to `backlog/` for automated pickup of the pure model port.
2. Re-scope: split the *pure struct + serde* port (WhisperResult/Segment/WordTiming
   with 3-decimal serde rounding) under a self-contained unit-test falsifier that
   builds its expected JSON inline (no `rust/fixtures/` dependency), then move the
   re-scoped item back to `backlog/`. The fixture-backed roundtrip becomes a
   separate, later item once fixtures land.
3. Delete if the stable-ts slice is no longer wanted.
