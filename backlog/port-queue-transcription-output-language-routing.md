# Port `transcribe_file` output-language + translation-routing decisions

**relates-to:** port-queue-transcription-service (the 9-skip-condition sibling)

## what
`TranscriptionService.transcribe_file` (`submate/queue/services/transcription.py`)
contains three pure-data decisions that the existing
`port-queue-transcription-service.md` item does NOT cover — it only pins the 9
skip conditions and the atomic subtitle write. These three decisions sit
*around* the whisper call and the LLM call but are themselves IO-free
`(config, audio_language, translate_to, source_language) -> ...` functions over
already-ported types (`LanguageCode`, the config subtitle/translation settings).
They decide *what* whisper/LLM are asked to do and *what* language the output is
labeled — getting them wrong mislabels every subtitle filename and routes
translation to the wrong engine. Port them as standalone functions so they are
testable without whisper inference.

1. **`use_whisper_translate`** — whether to use Whisper's built-in (free,
   English-only) translate instead of the LLM:
   `translate_to.is_some() && LanguageCode::from_string(translate_to) ==
   LanguageCode::ENGLISH`. (Python: `translate_to and
   LanguageCode.from_string(translate_to) == LanguageCode.ENGLISH`.) Routes
   through the ported `submate_lang` table, so `"en"`/`"eng"`/`"English"` all
   trigger it.

2. **Output-language resolution precedence** — the label applied to the
   subtitle filename, in this exact priority:
   - if `subtitle.force_detected_language_to` is non-empty → use it (verbatim);
   - else if `translate_to` is set → use `translate_to`;
   - else → use `source_language` (which is `audio_language` if the caller gave
     one, otherwise whisper's detected `result.language`).
   This `output_language` string then feeds `build_subtitle_path(..,
   language=output_language, naming_type=subtitle.language_naming_type,
   include_subgen_marker=.., include_model=.., model_name=whisper.model)` —
   already ported in `submate-paths`, so the falsifier can assert the full
   resulting subtitle path.

3. **Post-transcription LLM re-translation guard** — whether to run a *second*,
   LLM pass over the freshly written SRT:
   `translate_to.is_some() && !use_whisper_translate &&
   LanguageCode::from_string(translate_to) !=
   LanguageCode::from_string(source_language)`. (Python comment: "Compare
   normalized codes so different spellings of the same language (e.g.
   'spa'/'Spanish' vs Whisper's 'es') don't trigger a needless round-trip.")
   The *normalized* compare is load-bearing: `translate_to="spa"` with
   `source_language="es"` must NOT re-translate (both normalize to Spanish),
   whereas `translate_to="fr"` with `source_language="es"` must. Note also
   `target_language` for the **skip** decision is
   `from_string(translate_to)` if `translate_to` else `from_string(audio_language)`
   (top of `transcribe_file`) — port that resolution too so the skip-condition
   item gets the right `target_language` input.

## where
`rust/crates/submate-queue/src/lib.rs` (ingestion-side decision module, next to
the `port-queue-transcription-service` skip logic), as small pure functions
returning the routing booleans and the resolved `output_language` /
`target_language` strings. Reuse `submate_lang::LanguageCode::from_string` and
`submate_paths::build_subtitle_path`. No whisper/IO here.

## why
These decisions are faithful business logic that survives the Python→Rust
node-topology re-home unchanged (they run server-side at ingestion, same as the
skip logic). They are independent of `port-whisper-pipeline` /
`port-subtitle-detect` because they operate on already-resolved inputs
(`source_language`, `translate_to`, config) rather than performing inference,
so they can land *before* the whisper crate is wired — unblocking a slice of
`transcribe_file` parity that is otherwise gated behind the heavy whisper item.

## falsifies
`cargo test -p submate-queue parity::transcribe_routing` reproduces, against
`rust/fixtures/queue/transcribe_routing_cases.json`, for each
`(force_detected_language_to, translate_to, audio_language, source_language,
naming_type, include_subgen, include_model, model_name)` row:
`use_whisper_translate` (bool), `output_language` (string), the full
`build_subtitle_path` result (string), and `needs_llm_retranslation` (bool).
Must include the precedence rows (force-override beats translate_to beats
source) and the `spa`-vs-`es` normalized-equal no-retranslate row vs. the
`fr`-vs-`es` retranslate row, and an English `translate_to` row proving
`use_whisper_translate=true` AND `needs_llm_retranslation=false`.

**requires fixture: `rust/fixtures/queue/transcribe_routing_cases.json`
(capture first).** Does not exist; porter cannot write goldens (`rust/fixtures/`
denylisted). Add a capture to `rust/fixtures/capture/capture_queue_enums.py`
(or a new `capture_queue.py`) that imports `submate.queue.services.transcription`
+ `submate.config` (pure data, no external runtime — `nix develop --command
python3 -c 'import submate.queue.services.transcription'` succeeds) and, for the
case table, computes each decision exactly as `transcribe_file` does (extract
the three expressions verbatim) plus the `build_subtitle_path` result. Until
the golden lands the test self-skips (no-op pass) and arms when it appears.
