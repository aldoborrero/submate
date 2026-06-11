# Port the translation backend trait + chunking

**blocked-by:** port-config-schema

## what
Port `TranslationBackendBase` as a `Backend` trait and the chunked batch logic (chunk_size, separator-token join/split, fallback when block count mismatches) — backend-agnostic.

## where
`rust/crates/submate-translate/src/lib.rs`.

## why
Shared machinery for all four LLM backends; the chunking/separator logic is the real port surface.

## falsifies
`cargo test -p submate-translate parity::chunking` reproduces the Python chunk boundaries + separator tokens in `rust/fixtures/translate/chunking.json`.
