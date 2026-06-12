# Parity: submate-translate mocked-LLM apply (end-to-end SRT round-trip)

## what
Add the missing `parity::apply` test for `submate-translate`. The fixture
capture script (`rust/fixtures/capture/capture_translate.py`) names **two**
falsifier targets in its docstring — `parity::{chunking,apply}` — but only
`parity::chunking` exists (`rust/crates/submate-translate/tests/chunking.rs`).
The `apply` half is unwritten, so two golden fixtures are currently
**unconsumed by any test**:

- `rust/fixtures/translate/mock_llm.json` — recorded `{prompt: completion}`
  pairs from the deterministic stub.
- `rust/fixtures/translate/sampleA.out.srt` — the Python-produced golden output.

This is the contract's "mocked-LLM translation must match Python EXACTLY"
layer, and it is unverified.

## where
New `rust/crates/submate-translate/tests/apply.rs`, module `parity`, test
`apply`. It composes the crate's existing machinery
(`chunk_ranges` / `join_batch` / `split_batch` / `format_prompt` /
`TRANSLATION_PROMPT` / `SRT_SEPARATOR_TOKEN`) with SRT parse+serialize
(`submate-subtitle`) to reproduce `TranslationService.translate_srt_content`.

The flow to port (from `submate/translation.py`):
- `translate_srt_content`: `srt.parse` → `translate_subtitles` → `srt.compose`.
  Short-circuit `source_lang == target_lang` returns input unchanged (the
  fixture uses `en`→`es`, so this branch is NOT exercised).
- `translate_subtitles` → `_translate_chunk` → `_translate_batch`:
  `texts = [cue.content]`; `combined = "\n---BREAK---\n".join(texts)`;
  prompt = `format_prompt(TRANSLATION_PROMPT, "en", "es", combined)`;
  completion = `mock_llm.json[prompt]` (exact-key lookup, NO HTTP);
  `parts = [p.strip() for p in completion.split("---BREAK---")]`; on
  `len(parts) != len(texts)` keep originals; re-emit cues with the same
  index/start/end/proprietary and replaced content.

Default `chunk_size = 50`, so `sampleA` (3 cues) is a single batch.

## why
`mock_llm.json` records an IDENTITY-echo stub whose recorded completion equals
the full prompt (the capture's `PAYLOAD_MARKER = "Text:\n"` never matches the
template's `"Text to translate:\n"`, so `split(maxsplit=1)[-1]` returns the
whole prompt — confirm against the committed fixture rather than re-deriving).
That makes the `split_batch` / strip / count-match / SRT re-compose path the
load-bearing logic under test, none of which `parity::chunking` covers
(`chunking` stops at the join-string and never touches `split_batch`, response
handling, or SRT serialization). Any off-by-one in cue re-indexing, a stray
`\r\n` vs `\n`, a trailing-newline mismatch in `compose`, or a wrong strip
would silently diverge today.

## falsifies
`cargo test -p submate-translate parity::apply` exists and passes: driving
`rust/fixtures/translate/sampleA.in.srt` through the mocked-LLM apply flow
(completions served from `rust/fixtures/translate/mock_llm.json` by exact
prompt key) produces output **byte-for-byte equal** to
`rust/fixtures/translate/sampleA.out.srt` (assert via `parity::assert_str_eq`).

## blocked-by
Needs SRT parse+serialize from `submate-subtitle` (the crate currently exposes
no SRT reader/writer — the existing `chunking.rs` uses a throwaway test-local
SRT splitter). If `submate-subtitle` SRT round-trip is not yet available, land
that first or have the test parse/compose the 3-cue fixture with a documented
test-local reader/writer as `chunking.rs` already does for the input side.
