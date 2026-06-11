# stable-ts (D): to_srt_vtt output formatting

**blocked-by:** port-stablets-suppress-apply-C2

## what
Port `to_srt_vtt`, `sec2srt`/`sec2vtt` timestamp formatting (comma vs period decimals), and word-level highlight tagging (`<font color>` for SRT, cue timing markers for VTT), including gap-filling between words.

## where
`rust/crates/stable-ts/src/output.rs`.

## why
Final subtitle emission; must be byte-identical to Python.

## falsifies
`cargo test -p stable-ts parity::output` emits SRT/VTT byte-identical to `rust/fixtures/stablets/*/03.srt` and `03.vtt` from `02_suppress.json`.
