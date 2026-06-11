# Port SRT/VTT parsing + round-trip

## what
Hand-roll SRT and VTT cue parsing + serialization matching the Python `srt` + `pysubs2` output byte-for-byte (needed so translation re-emits identical files).

## where
`rust/crates/submate-subtitle/src/lib.rs`.

## why
Translation parses and re-emits these; byte-parity avoids spurious diffs.

## falsifies
`cargo test -p submate-subtitle parity::srt_roundtrip` re-emits each `rust/fixtures/subtitle/*.srt` byte-identically.
