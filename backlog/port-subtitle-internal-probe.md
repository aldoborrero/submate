# Port internal (embedded) subtitle-track probing via ffprobe

**blocked-by:** port-subtitle-discovery-fs (the OR-combinator
`has_subtitle_language` needs the external half from that item)

**supersedes (partial):** carves the PyAV/internal-probe half out of the
over-broad `port-subtitle-detect` umbrella (which said "internal-subtitle
detection (PyAV) can shell out to ffprobe" but bundled it with the pure-data
filename helpers under one vague falsifier).

## what
Port the embedded-subtitle-stream language probe from `submate/subtitle.py`.
Python uses PyAV (`av.open(...).streams`, keeping `stream.type == "subtitle"`
and reading `stream.metadata["language"]`); the Rust port shells out to
`ffprobe` exactly as submate-media's `get_audio_tracks` already does (reuse that
crate's ffprobe-invocation + `-show_streams -of json` parsing convention, just
filtering `codec_type == "subtitle"` instead of `audio`).

- `get_internal_subtitle_languages(file_path) -> Vec<LanguageCode>`
  - for each subtitle stream, take `tags.language` (Python
    `stream.metadata.get("language", "")`) and map via
    `LanguageCode::from_iso_639_2(...)`, falling back to `LanguageCode::None`
    when the tag is absent/empty or unmappable (Python:
    `from_iso_639_2(lang_code) or LanguageCode.NONE`).
  - **swallow every error** (probe failure, missing file, demux error) →
    return `[]`. Python wraps the whole body in `except Exception` and returns
    `[]`; mirror that exact-empty fallback (same posture as submate-media's
    `get_audio_languages`).
  - order: one entry per subtitle stream, in stream order (contractual — it is a
    `Vec`, and the falsifier compares the list).
- `has_internal_subtitle_language(video_path, language) -> bool` —
  `language in get_internal_subtitle_languages(video_path)`.
- `has_any_internal_subtitle(video_path) -> bool` —
  `!get_internal_subtitle_languages(video_path).is_empty()`.
- `has_subtitle_language(video_path, language, only_subgen=false) -> bool` —
  the OR-combinator: internal is checked first **only when** `!only_subgen`
  (internal tracks can't be "subgen"), then
  `has_external_subtitle_language(video_path, language, only_subgen)`.

## where
`rust/crates/submate-subtitle/src/lib.rs`, adding the ffprobe path. Reuse the
submate-media ffprobe convention (add submate-media or a shared probe helper as
a dep) rather than re-rolling the subprocess plumbing.

## why
`has_subtitle_language` (internal OR external) is the exact predicate the 9-way
queue skip decision (`port-queue-transcription-service`) calls; the external +
LRC half lands in `port-subtitle-discovery-fs`, this item closes the internal
half so the combinator is whole.

## falsifies
`cargo test -p submate-subtitle parity::internal_probe` probes a clip with two
tagged subtitle streams (e.g. `eng`, `spa`) and one untagged, asserting the
returned `LanguageCode` list **exactly** equals the golden
`rust/fixtures/subtitle/clipS.subs.json` (`[English, Spanish, None]`), and that
`has_subtitle_language` returns the documented internal-vs-external/`only_subgen`
matrix. Gate the probe behind an `ffprobe`-on-PATH check and skip (don't fail)
when absent, matching submate-media's `extract_pcm_sha` test posture.

**requires fixture: rust/fixtures/subtitle/clipS.{mkv,subs.json} (capture
first)** — a tiny clip muxed with subtitle tracks tagged `eng`/`spa` plus one
untagged stream, and `subs.json` dumped from the Python
`get_internal_subtitle_languages` (PyAV) reference run. I cannot touch
`rust/fixtures/**` (denylisted); flag for the capture harness / a human. Needs
ffmpeg to mux the clip but no credentials.
