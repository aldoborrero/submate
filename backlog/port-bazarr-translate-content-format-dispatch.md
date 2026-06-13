# Port BazarrService._translate_content format dispatch + skip conditions

**blocked-by:** none (pure-data: takes already-formatted subtitle content +
source/target lang + an `OutputFormat`, calls the mocked translate helpers.
Independent of whisper — the *transcribe* half of `BazarrService` is the
whisper-blocked `port-queue-bazarr-service`; this is only the post-transcription
translate dispatch and is separable from it.)

## what

Port `BazarrService._translate_content` from
`submate/queue/services/bazarr.py`: the per-format dispatch that decides which
translation method to apply to already-formatted Bazarr output (and when to skip
translation entirely). The pure logic is:

1. **Short-circuit guard** — `if not content or source_lang == target_lang:
   return content` (empty string or same-language returns the input unchanged,
   *before* constructing any translation service).
2. **Format dispatch** (`match output_format`):
   - `SRT`  → `translate_srt_content(content, source, target)`
   - `VTT`  → `translate_vtt_content(content, source, target)`
   - `TXT`  → `translate_text(content, source, target)` (plain-text path, the
     default `TRANSLATION_PROMPT`, no separator-token batching)
   - `JSON` → **skip**: log a warning, return `content` unchanged
     ("Translation not supported for JSON format").
   - `_`    → return `content` unchanged.
3. **Exception fallback** — any exception raised while translating is caught,
   logged, and the **original** `content` is returned (translation failure must
   never propagate to the Bazarr caller; it degrades to the untranslated text).

The three live methods (`translate_srt_content`, `translate_vtt_content`,
`translate_text`) are already ported in `submate-translate` as closure-driven
fns; this item is purely the dispatch+guard+fallback wrapper around them. Note
`translate_text` is the only path that uses the bare `TRANSLATION_PROMPT` rather
than the `---BREAK---` batch separator — verify the Rust `submate-translate`
exposes a plain-text translate entrypoint (the SRT/VTT fns wrap batching); if
not, expose `translate_text` mirroring `TranslationService.translate_text`
(same-lang short-circuit returns input; otherwise one `backend.translate` call
with `prompt_template=None`).

## where

`rust/crates/submate-bazarr/src/` — add a `translate_content` fn taking
`content: &str`, `source_lang`, `target_lang`, `output_format: OutputFormat`
(from `submate-queue::models`), and a `complete: &mut dyn FnMut(&str) ->
Result<String, E>` closure for the mocked LLM, returning the dispatched result.
The JSON-skip and exception-fallback branches return `content` verbatim. If a
plain-text translate entrypoint is missing, add `translate_text` to
`submate-translate` first (small, in the same item).

## why

`_translate_content` is the only place Bazarr's "translate if target differs
from detected" feature decides *how* to translate per output format and *when
to skip*. Getting the dispatch wrong (e.g. running the SRT chunked-batch
translator over a TXT blob, or attempting to translate a JSON dump) corrupts the
subtitle Bazarr receives. The skip + fallback guarantees (same-lang no-op, empty
no-op, JSON no-op, exception→original) are user-visible contract: Bazarr must
get back well-formed content for the requested format even when translation is
unavailable or fails.

## falsifies

`cargo test -p submate-bazarr parity::translate_content_dispatch` — a table of
cases driven by a recorded mock-LLM map reproduces Python's
`_translate_content` output byte-for-byte for each format:

- SRT in / SRT out (chunked-batch path, `---BREAK---` separator)
- VTT in / VTT out (pysubs2 cue path, `|||SUBTITLE_BREAK|||` separator)
- TXT in / TXT out (plain `TRANSLATION_PROMPT`, single request)
- JSON in → identical JSON out (skip, no LLM call recorded)
- same source==target lang → identical content out (no LLM call recorded)
- empty content → empty out (no LLM call recorded)

requires fixture: `rust/fixtures/translate/bazarr_translate_content.json`
(capture first — denylisted). Extend `rust/fixtures/capture/capture_translate.py`
to instantiate `BazarrService` with a stub backend whose `translate` is recorded
prompt→response, drive `_translate_content` for the six cases above (en→es,
plus the same-lang/empty/json no-op cases), and dump `{cases: [...], mock_llm:
{prompt: response}}` so the Rust test can replay the recorded `complete`
closure. Until that golden lands this item is fixture-blocked. (The
mock_llm sub-map mirrors the existing `rust/fixtures/translate/mock_llm.json`
prompt→response shape.)
