# Port `submate translate` filename/language pure-data helpers

**blocked-by:** none (submate-lang and submate-paths already ported; this slice
touches neither the LLM service nor clap wiring)

## what
Port the three deterministic helper functions from
`submate/cli/commands/translate.py` that decide *which* files to translate,
*what* source language to assume, and *where* to write output. These are
pure-data, exact-match, and depend only on already-ported crates — yet the
umbrella `port-cli-commands` (blocked behind whisper-pipeline / node-agent /
translate-srt-apply) does NOT decompose them, leaving real tested Python
behavior unfalsifiable. Split it out now, exactly as `port-cli-config-show`
carved the `config show` rows out of the same blocked umbrella.

Three functions to port byte-for-byte (module constant `SUBTITLE_EXTENSIONS =
{".srt", ".vtt", ".ass", ".ssa"}`):

- `is_subtitle_file(path) -> bool` — `path.suffix.lower() in SUBTITLE_EXTENSIONS`.
  Suffix is the Python `Path.suffix` (last `.`-segment, lowercased), so
  `movie.SRT` matches, `movie.tar.gz`→`.gz` does not, a dotfile like `.srt`
  with no stem has suffix `""` → false (mirror `pathlib.PurePath.suffix`).

- `detect_source_language(file, source_lang) -> str` — resolve the source
  language for a subtitle file:
  - if `source_lang != "auto"`: return `source_lang` unchanged (explicit wins).
  - else if `"." in file.stem`: take `candidate = file.stem.rsplit(".", 1)[-1]`
    (last dotted stem segment), and return it ONLY IF
    `LanguageCode::from_string(Some(candidate))` is NOT `LanguageCode::None`;
    otherwise fall back to `"en"`. So `movie.fr.srt`→`"fr"`, but
    `movie.v2.srt`→`"en"` and `episode.01.srt`→`"en"` (non-language tokens are
    rejected, not passed to the translator). Note `stem` excludes the final
    suffix: for `movie.fr.srt`, `stem == "movie.fr"`, candidate == `"fr"`.

- output-path derivation (currently inline in the `for file in files:` loop;
  extract it as a pure function, e.g. `output_path(file, target_lang) ->
  PathBuf`, NOT the `--output` override branch which is trivial IO):
  - `stem = file.stem`; if `"." in stem`: `base = stem.rsplit(".", 1)[0]`
    (strip the existing trailing dotted segment — a language suffix), else
    `base = stem`; result = `file.parent / f"{base}.{target_lang}{file.suffix}"`.
  - So `movie.srt -t es` → `movie.es.srt`; `movie.en.srt -t es` →
    `movie.es.srt` (the `.en` is replaced, NOT appended — this strips ANY
    trailing dotted segment, including non-language ones like `movie.v2.srt -t
    es` → `movie.es.srt`; preserve that, it matches Python exactly). `file.suffix`
    keeps the original extension and case as-is.

`find_subtitle_files(path, recursive)` (glob `**/*` vs `*`, filter
`f.is_file() and is_subtitle_file(f)`) is filesystem-walking IO; if you port it
too, it is the only part needing a tmp-dir, and its falsifier is the
deterministic *sort/filter* of a given file list, not the OS walk. Keep it
optional / out of the pure golden if it complicates the slice.

## where
`rust/crates/submate-cli/src/translate_paths.rs` (new module; three pure
functions, no clap/IO). Reuse `LanguageCode::from_string` from `submate-lang`.
Wire into the `translate` subcommand later under `port-cli-commands`; this
module must compile and test standalone without the rest of the CLI.

## why
These three helpers are the user-visible "did it pick the right language and
write to the right filename" contract. A drift — appending `.es` instead of
replacing `.en`, accepting `v2` as a language, or a `.SRT` case-fold miss —
silently corrupts output filenames and feeds garbage source languages to the
translator. They are exact-match pure-data with existing Python tests
(`tests/test_cli.py::test_detect_source_language_uses_valid_tag`,
`::test_detect_source_language_rejects_non_language_token`,
`::test_detect_source_language_respects_explicit`, and the `is_subtitle_file`
assertions around line 136-146), and depend only on the already-ported
`submate-lang` table — no reason to wait on the whole blocked CLI.

## falsifies
`cargo test -p submate-cli translate_paths` drives a golden table of cases
through `is_subtitle_file`, `detect_source_language`, and `output_path` and
asserts each row exactly (via `assert_json_eq`). Cases MUST include, at minimum:
`movie.fr.srt`/auto → `fr`; `movie.v2.srt`/auto → `en`; `episode.01.srt`/auto →
`en`; `movie.fr.srt`/`es` → `es` (explicit); `movie.SRT` is-subtitle → true;
`movie.tar.gz` is-subtitle → false; output `movie.srt`+`es` → `movie.es.srt`;
output `movie.en.srt`+`es` → `movie.es.srt`; output `movie.v2.srt`+`es` →
`movie.es.srt`.

requires fixture: `rust/fixtures/cli/translate_filename_cases.json` (capture
first) — a list of `{file, source_lang, target_lang, is_subtitle,
detected_source, output_path}` rows produced by calling the three Python helpers
over the case set above. Capture by importing
`submate.cli.commands.translate.{is_subtitle_file, detect_source_language}` and
the inline output-path logic (factor it into a small `_output_path` helper in
the capture script, mirroring the loop body exactly) and dumping the rows in
order. I cannot touch `rust/fixtures/` (denylisted) — flag for a human/capture
run; add a `capture/capture_cli_translate.py` alongside the existing
`capture_*.py` scripts following their `_common.py` pattern.
