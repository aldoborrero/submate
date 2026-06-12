# cli-audio: typed `--audio` selector (lang / track:N / default / auto)

**blocked-by:** cli-audio-track-disposition

## what
Replace the bare `--audio-language` semantics with a small **typed selector** on
`-a`/`--audio` so track selection survives heterogeneous batches and untagged
files. Accept exactly these forms (closed grammar — do NOT grow into a query
language):

| value | meaning |
|---|---|
| `ja` / `lang:ja` | first audio track whose language tag matches (case-insensitive) |
| `track:2` | the audio track at that 0-based audio-stream index |
| `default` | the container's default-disposition track |
| `auto` (or omitted) | smart default: 1 track → it; else default-flagged track; else track 0 |

Introduce `enum AudioSelector { Lang(String), Index(usize), Default, Auto }`
with a `FromStr`, and a resolver
`fn resolve_audio_selector(tracks: &[AudioTrack], sel: &AudioSelector) -> Result<usize, SelectError>`
returning the chosen `AudioTrack.index`. Resolution rules:
- `Lang` → first matching track; **no match → error** listing available langs.
- `Index` → bounds-checked; out of range → error naming the valid range.
- `Default` → the `default==true` track; none flagged → fall back to track 0.
- `Auto` → single track → it; else the `default==true` track; else track 0.

When several tracks match a `Lang` selector (e.g. two `jpn` tracks), the resolver
picks the first **and** the caller logs a one-line note that it was ambiguous
(the interactive picker in cli-audio-interactive-picker upgrades this to a
prompt on a TTY). Keep `--audio-language` working as a hidden deprecated alias
that maps to `Lang(..)`.

Resolution stays in / is shared with the media layer used by
`prepare_audio_for_transcription` so the queued (non-`--sync`) path selects the
same track; the resolver itself must be a pure, unit-testable function.

## where
- `rust/crates/submate-cli/src/main.rs` — `AudioSelector` + `FromStr`, wire `-a`
  to parse it; keep `--audio-language` as a hidden alias.
- `rust/crates/submate-media/src/lib.rs` — `resolve_audio_selector` (pure) and
  thread it through `prepare_audio_for_transcription` in place of the current
  "language → `get_audio_track_by_language`, else `tracks[0]`" logic.

## why
`-a ja` is fine until a file has two `jpn` tracks, an untagged track, or you
need the commentary track — then there is no way to say "track 2" or "the
default one". The engine already extracts by index
(`extract_audio_track_to_memory(path, index)`); only the selection vocabulary is
missing.

## falsifies
`cargo test -p submate-media` (and `-p submate-cli` for the `FromStr`) green,
including `audio_selector_*` cases:
- `"lang:ja"`/`"ja"` parse to `Lang("ja")`; `"track:2"` → `Index(2)`;
  `"default"` → `Default`; `"auto"`/empty → `Auto`; a malformed value errors.
- resolve: `Lang("ja")` over two `jpn` tracks returns the first; `Index(9)` on a
  3-track file errors; `Default` with no default-flag falls back to index 0;
  `Auto` on a multi-track file returns the default-flagged index.
