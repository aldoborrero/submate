# Port translated-chunk application onto cues

**blocked-by:** port-translate-backend-trait, port-subtitle-srt-vtt-parse

## what
Port applying translated chunks back onto SRT/VTT/ASS cues, preserving ASS tags and structure (translate_srt_content / translate_vtt_content / translate_ass_content).

## where
`rust/crates/submate-translate/src/lib.rs`.

## why
Produces the final translated subtitle file.

## falsifies
`cargo test -p submate-translate parity::apply` transforms `rust/fixtures/translate/*.in.srt` + `mock_llm.json` → `*.out.srt` exactly.
