# Port on-disk subtitle discovery + filename-language parsing (pure-data + fs)

**blocked-by:** none (submate-lang is ported: `LanguageCode::from_string`,
`from_iso_639_1`, `from_iso_639_2` all exist; this slice touches neither PyAV
nor any format parser)

**supersedes (partial):** carves the filesystem-scan + filename-language +
LRC behavior out of the over-broad `port-subtitle-detect` umbrella, which lumps
SUBTITLE_EXTENSIONS, `parse_subtitle_language`, *all* `has_*` helpers, AND the
PyAV internal probe into one item behind a single vague `*.detected.json`
falsifier that maps to none of these. This is the same pure-data carve-out
pattern as `port-cli-translate-filename-logic` / `port-cli-config-show`. The
PyAV/ffprobe half is split into `port-subtitle-internal-probe`.

## what
Port the deterministic disk-discovery and filename helpers from
`submate/subtitle.py`. These depend only on the directory listing, the filename,
and the already-ported language table — no media demux, no subtitle-format
parsing. Port them byte-for-byte, mirroring the documented edge cases:

Module constant (exact, note this is the WIDE set used for discovery — distinct
from the translate-path `SUBTITLE_EXTENSIONS` in `cli/commands/translate.py`):

```
SUBTITLE_EXTENSIONS = {".srt", ".vtt", ".sub", ".ass", ".ssa",
                       ".idx", ".sbv", ".pgs", ".ttml", ".lrc"}
```

- `get_external_subtitle_paths(video_path) -> Vec<PathBuf>`
  - returns `[]` if `video_path` does not exist.
  - lists `video_path.parent`, keeps regular files whose `suffix.lower()` is in
    `SUBTITLE_EXTENSIONS`.
  - keeps a file iff `file.stem == video_stem` OR
    `file.stem.starts_with(video_stem + ".")` — the **dot boundary** that stops
    `Episode 10.en.srt` matching video `Episode 1` (test
    `test_external_paths_reject_prefix_collision`). `stem`/`suffix` follow
    `pathlib.PurePath` semantics (last `.`-segment); use the same definition the
    already-ported submate-paths uses so they agree.
  - directory-scan order is NOT contractual — the parity falsifier compares the
    result as a **set** of file names (Python uses `iterdir()`, unordered).

- `parse_subtitle_language(subtitle_path, video_stem) -> LanguageCode`
  - if `stem != video_stem` and not `stem.starts_with(video_stem + ".")` →
    `LanguageCode::None` (dot-boundary guard, `test_parse_requires_dot_boundary`,
    `test_parse_unrelated_stem_returns_none`).
  - `suffix = stem[len(video_stem)..].trim_start_matches('.')`; empty → `None`
    (`test_parse_no_language_returns_none`).
  - split `suffix` on `.`, iterate **reversed**, return the first segment whose
    `LanguageCode::from_string(Some(seg))` is not `None`; else `None`. The
    reversed scan is load-bearing: `movie.no.forced.en` must resolve to English,
    not Norwegian (`no`) — `test_parse_prefers_trailing_tag_over_earlier_collision`
    — and `movie.en.forced` resolves English via the earlier segment once the
    trailing `forced` fails (`test_parse_trailing_flag_after_language`).

- `has_external_subtitle_language(video_path, language, only_subgen=false) -> bool`
  - over `get_external_subtitle_paths`, optionally skipping files whose
    `stem.to_lowercase()` lacks `"subgen"` when `only_subgen`, return true iff
    any `parse_subtitle_language(sub, video_stem) == language`.
- `has_any_external_subtitle(video_path) -> bool` —
  `!get_external_subtitle_paths(video_path).is_empty()`.
- `get_lrc_path(audio_path) -> PathBuf` — `audio_path.with_suffix(".lrc")`
  (pathlib `with_suffix`: replaces the last suffix, adds one if none).
- `has_lrc_file(audio_path) -> bool` — `get_lrc_path(audio_path).exists()`.

## where
`rust/crates/submate-subtitle/src/lib.rs` (currently a 3-line stub). Pure
std + submate-lang; no new deps.

## why
The queue/server skip logic (`port-queue-transcription-service`, the 9 skip
conditions) calls `has_subtitle_language`, which is `has_internal OR
has_external`; the external half plus the LRC check are pure-data and unblockable
now, so they should not wait behind the PyAV probe or the format `.detected.json`
capture.

## falsifies
`cargo test -p submate-subtitle parity::discovery_fs` drives
`rust/fixtures/subtitle/discovery_cases.json` — each case is
`{video_stem, dir_files:[...], expect_external:[...], expect_parse:{file->lang}}`
plus the `with_suffix(".lrc")` cases — asserting **exact** set-equality on the
returned external file names and exact `LanguageCode.value` on each parse, built
in a `tmp_path`-style temp dir (mirror `test_subtitle.py`).

**requires fixture: rust/fixtures/subtitle/discovery_cases.json (capture first)**
— author via a `capture/capture_subtitle_discovery.py` that lays out the
`test_subtitle.py` scenarios (`Episode 1` vs `Episode 10`, `movie.no.forced.en`,
`movie.en.forced`, `movie.subgen.medium.en`, an `.lrc` sibling) in a temp dir
and dumps inputs→outputs to JSON. I cannot touch `rust/fixtures/**` (denylisted);
flag for the META capture pre-pass / a human. This is a pure-data capture (no
credentials, no GPU) — do NOT park to `needs-human/`.

---

**META unpark (round 2, 2026-06-12):** abandoned this round as a "denylist
scope violation" (the porter touched `rust/fixtures/capture/capture_subtitle_discovery.py`)
and rerouted to `needs-human/`. That is the wrong destination: the denylist
hit is the *capture-script authoring* step, which the item itself says belongs
to the **capture pre-pass**, not to a human credential gate. There is no
external runtime here — the scenarios are a temp-dir + filename layout. Same
pattern that landed `port-bazarr-pcm-wav-wrap` cleanly (its capture script +
goldens were authored in a deliberate capture commit, then the port diffed
against them). Returned to `backlog/`. Next round's capture pre-pass must
author `rust/fixtures/capture/capture_subtitle_discovery.py` and land
`rust/fixtures/subtitle/discovery_cases.json` in a dedicated capture commit
**before** the porter is dispatched, so the porter never touches the oracle.
Do NOT re-park to `needs-human/`.
