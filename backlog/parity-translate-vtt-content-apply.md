# Parity: mocked-LLM VTT content apply matches the Python golden

## what
Add a golden-fixture parity falsifier for the WebVTT translate-and-reserialize
flow `submate_translate::translate_vtt_content` (ports
`TranslationService.translate_vtt_content` in `submate/translation.py`).

The function is already implemented in
`rust/crates/submate-translate/src/lib.rs` (parses with
`submate_subtitle::cue::parse_vtt`, joins cues with the
`|||SUBTITLE_BREAK|||` separator under `TRANSLATION_PROMPT`, reserializes with
`compose_vtt`). What is missing is a parity test proving the *serialized* output
matches the Python golden byte-for-byte. The existing `tests/apply.rs` covers
only the SRT path (`sampleA.in.srt` -> `sampleA.out.srt`); there is no VTT
in/out golden and no test driving this function.

This matters because Python serializes VTT with **pysubs2**
(`subs.to_string("vtt")`) while the Rust port uses the workspace `compose_vtt`.
The two serializers can diverge on header (`WEBVTT`), cue numbering, blank-line
placement, and dot-separated millisecond stamps — exactly the byte-for-byte
output-formatting contract the port must hold. Without a golden this divergence
is invisible.

Falsifier-driving details, all concrete:
- separator token is `|||SUBTITLE_BREAK|||` (NOT the SRT `---BREAK---`), so the
  recorded prompt keys differ from `mock_llm.json`;
- prompt template is `TRANSLATION_PROMPT` (same as SRT), formatted via
  `format_prompt`;
- `chunk_size` default is `50` (`TranslationSettings.chunk_size`), so a small
  fixture is a single batch;
- when `parse_vtt` yields no cues the input is returned unchanged (cover with a
  header-only / cue-less VTT case).

## where
Test: new `rust/crates/submate-translate/tests/apply.rs` test case (or a sibling
`vtt_apply.rs`), mirroring the existing SRT `apply()` test's mocked-backend
wiring (exact-key `{prompt: completion}` lookup from a JSON map, no HTTP).

Requires fixtures: `rust/fixtures/translate/sampleVtt.in.vtt`,
`rust/fixtures/translate/sampleVtt.out.vtt`, and
`rust/fixtures/translate/mock_llm_vtt.json` (capture first) — the prompt/
completion map for the `|||SUBTITLE_BREAK|||`-joined batch. The `.out.vtt`
golden MUST be produced by running Python `translate_vtt_content` end-to-end so
it captures the exact `pysubs2.to_string("vtt")` byte layout. Scout cannot
write under `rust/fixtures/` (denylisted) — flag for human/capture.

## why
The VTT translate apply flow is implemented but its byte-for-byte parity to the
Python pysubs2 serialization is unverified. A passing SRT golden does not cover
the distinct VTT separator, header, and serializer.

## falsifies
`cargo test -p submate-translate vtt_apply` — `translate_vtt_content` over
`rust/fixtures/translate/sampleVtt.in.vtt`, with completions served from
`rust/fixtures/translate/mock_llm_vtt.json` and `chunk_size = 50`, recomposes a
VTT string equal to `rust/fixtures/translate/sampleVtt.out.vtt` byte-for-byte
(`parity::assert_str_eq`). The header-only / cue-less input returns unchanged.
