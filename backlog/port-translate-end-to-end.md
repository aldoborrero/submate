# port-translate: end-to-end `transcribe --translate-to`

**blocked-by:** port-translate-backend-factory

## what
Make `submate transcribe --translate-to <lang> movie.mkv` produce a translated
subtitle in one step. Today `--translate-to` sets `JobOpts.target_language`, but
the node never translates, so the output is the untranslated transcription.

Wire translation as a **post-transcription step in the job layer** (NOT inside
`whisper_processor`), so it is testable without the `model` feature and composes
with any `JobProcessor`:

1. When a job carries `target_language` (and a source language is known/auto),
   after the processor returns the assembled subtitle string, translate it with
   `submate-translate` before the result is reported:
   - `.srt` → `translate_srt_content`, `.vtt` → `translate_vtt_content`,
     `.ass` → `translate_ass_dialogue` — dispatched by the job's output format.
   - chunk size + prompt come from config (`translation.chunk_size`, the prompt
     template); the backend is a `Box<dyn Backend>` provided to the node.
2. The backend is supplied at node construction: the `--sync` embedded node gets
   it from the CLI (built via `submate_translate::make_backend` from
   port-translate-backend-factory); a standalone `submate node` builds it from
   its own config the same way.
3. The CLI writes the translated output under the **language-suffixed** path
   (`translate_paths::output_path(file, target_lang)` → `movie.<lang>.srt`)
   when `--translate-to` is set, instead of the plain `movie.srt`.

No translation requested ⇒ behavior is byte-identical to today (plain
transcription, unsuffixed path).

## where
- `rust/crates/submate-node/src/lib.rs` — a translation post-step in the
  agent/result path keyed on `JobOpts.target_language`; the node holds an
  optional `Box<dyn Backend>` + chunk size. Keep `whisper_processor`
  transcription-only.
- `rust/crates/submate-cli/src/main.rs` — build the backend (shared factory) and
  pass it to the embedded node (`make_processor`/`spawn_embedded_node`);
  language-suffixed output naming in the `--sync` write path when translating.
- `rust/crates/submate-proto` — `JobOpts` already carries `target_language`;
  ensure the output format needed for dispatch is available to the step.

## why
This is the missing half of submate's core value (Whisper **+ LLM translation**)
and the primary JA→ES workflow. The translation library, the four backends, and
the standalone `submate translate` command already exist; only the inline
transcribe→translate path is unwired.

## falsifies
`cargo test -p submate-node` green, including `translate_post_step`:
- Drive the job layer with a **stub `JobProcessor`** returning a fixed SRT and a
  **stub `Backend`** that deterministically transforms text (e.g. uppercases each
  batch). A job with `target_language = Some("es")` yields a subtitle whose cue
  text is transformed; the same job with `target_language = None` yields the
  fixed SRT unchanged. (No `model` feature, no network.)
- `cargo test -p submate-cli translate_output_path`: with `--translate-to es`,
  the `--sync` write path targets `movie.es.srt` (language-suffixed), and the
  plain transcribe path still targets `movie.srt`.
