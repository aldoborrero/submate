# Parity: node ASS full-file translate round-trip matches a Python golden

**blocked-by:** parity-translate-ass-dialogue-apply

## what
`submate-node` landed a NEW full-file ASS translate round-trip,
`translate_ass_content` in `rust/crates/submate-node/src/lib.rs` (called from
`TranslationStep::translate` for `OutputFormat::Ass`). It walks `[Events]`
`Dialogue:` lines, treats the text as everything after the 9th comma
(`nth_comma_end(rest, 9)`), batch-translates those texts through
`submate_translate::translate_ass_dialogue`, and **splices the new text back
in place, copying every other byte of the input verbatim**
(`split_inclusive('\n')` + `split_line_ending`, so no trailing newline is
invented or dropped).

This is the full `pysubs2.SSAFile.from_string(...) -> to_string("ass")`
round-trip that the existing `parity-translate-ass-dialogue-apply.md` item
EXPLICITLY deferred as out of scope ("the full pysubs2 round-trip belongs to a
future ASS-serializer item"). It has now partially landed in `submate-node`
with only a hand-rolled smoke test
(`translate_post_step_ass_preserves_layout`, asserts `out.contains(...)` with
an `UpperBackend`), and **no golden-fixture parity test** pinning it to the
Python `translate_ass_content` output.

## divergence risk (why this is parity-critical, not just a missing test)
The Rust path is byte-preserving splice-in-place. Python
(`submate/translation.py::translate_ass_content`) re-emits the WHOLE file via
`subs.to_string("ass")`. pysubs2's serializer does NOT echo input bytes — it
re-renders from its parsed model, so against a Python golden the two can differ
even when no text changed, e.g.:
- `[Script Info]` block normalization / injected default fields
  (`ScriptType: v4.00+`, `WrapStyle`, `ScaledBorderAndShadow`, etc.);
- `Format:` line field casing/spacing normalization;
- `Dialogue:` field re-spacing (pysubs2 emits `Dialogue: 0,0:00:00.00,...`
  with its own spacing/time precision, e.g. centiseconds `0:00:00.00`);
- event ordering / `Comment:` vs `Dialogue:` handling;
- trailing-newline conventions.
The Rust "9th comma = text" heuristic also assumes the input's own field
layout; a golden captured from pysubs2 may carry a different layout than the
input, so a naive in-place splice cannot reproduce it. This must be falsified
against a captured golden before the node ASS path can be called done.

## where
Test: new golden-fixture parity test in `submate-node` (a `tests/ass_apply.rs`
integration test, or a `parity::` unit test), mirroring the SRT mocked-backend
wiring in `submate-translate/tests/apply.rs` (exact-key `{prompt: completion}`
lookup, no HTTP). `translate_ass_content` is currently private — either expose
it (`pub(crate)` + an integration entry, or drive it through
`TranslationStep::translate` with `OutputFormat::Ass`).

Requires fixtures (capture first — `rust/fixtures/` is denylisted for grind
implementers, so flag for human/capture):
- `rust/fixtures/translate/sampleA.in.ass` — an ASS document with a real
  `[Script Info]`/`[V4+ Styles]`/`[Events]` structure and at least:
  one plain `Dialogue:` line, one with `{...}` override tags translated
  cleanly, one whose recorded completion ALTERS a tag (keep-original branch),
  and one non-`Dialogue` event line (e.g. `Comment:`) that must pass through.
- `rust/fixtures/translate/mock_llm_ass.json` — the
  `ASS_TRANSLATION_PROMPT` `{prompt: completion}` map for the batch (separator
  `|||SUBTITLE_BREAK|||`, `chunk_size = 50`).
- `rust/fixtures/translate/sampleA.out.ass` — the authoritative golden,
  produced by running `submate/translation.py::translate_ass_content` (real
  pysubs2 `to_string("ass")`) over `sampleA.in.ass` with completions served
  from `mock_llm_ass.json`. This captures pysubs2's actual re-emission so the
  Rust splice can be judged against it.

## why
The byte-preserving in-place splice is a deliberate shortcut around having no
ASS serializer. Whether it actually reproduces pysubs2's `to_string("ass")` is
UNVERIFIED — the only test asserts substrings, never byte-equality to a Python
golden. If pysubs2 normalizes anything (it does, at minimum `[Script Info]`),
the node ASS output diverges from the spec and a downstream Bazarr/Jellyfin
consumer gets a structurally different file than the Python tool would emit.

## falsifies
`cargo test -p submate-node ass_apply` — driving the node ASS translate
round-trip (`translate_ass_content` / `TranslationStep::translate` with
`OutputFormat::Ass`) over `rust/fixtures/translate/sampleA.in.ass`, completions
from `rust/fixtures/translate/mock_llm_ass.json`, `chunk_size = 50`, returns a
string equal to `rust/fixtures/translate/sampleA.out.ass` byte-for-byte
(`parity::assert_str_eq`). If pysubs2 re-emission differs from the in-place
splice, this test fails and pins the exact bytes an implementer must reproduce
(or proves the splice-shortcut is insufficient and an ASS serializer is needed).
