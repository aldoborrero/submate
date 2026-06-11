# Port subtitle format detection

**blocked-by:** port-lang-enum

## what
Port subtitle format/extension detection + language-from-filename parsing from `submate/subtitle.py` (SUBTITLE_EXTENSIONS, parse_subtitle_language, has_*_subtitle helpers). Internal-subtitle detection (PyAV) can shell out to ffprobe.

## where
`rust/crates/submate-subtitle/src/lib.rs`.

## why
The queue's skip logic depends on accurate subtitle detection.

## falsifies
`cargo test -p submate-subtitle parity::detect` passes exact-match against `rust/fixtures/subtitle/*.detected.json`.
