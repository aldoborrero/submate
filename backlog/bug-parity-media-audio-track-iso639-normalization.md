# bug-parity: media audio-track matching ISO-639-normalizes; Python does NOT

**relates-to:** submate-media `get_audio_track_by_language`,
`resolve_audio_selector`, `lang_match_is_ambiguous`,
`parity::audio_track_language_normalizes`,
`parity::audio_selector_lang_picks_first_of_several_matches`

**introduced-by:** `bb9b252 Normalize ISO 639 codes when matching audio tracks`
(merged via `bed4f43`), in `rust/crates/submate-media/src/lib.rs`.

## the divergence

The Rust port now routes every audio-track language comparison through
`LanguageCode::from_string(...)` (the new `track_language_matches` helper), so
`ja`↔`jpn`, `en`↔`eng`, language names, and case all fold to the same enum
before comparing. The Python SPEC does **no normalization** — it is a literal
case-folded string equality.

Python `submate/media.py` (`get_audio_track_by_language`, the spec):

```python
language_lower = language.lower()
for track in tracks:
    if track.language.lower() == language_lower:
        return track
return None
```

There is no `LanguageCode` involved anywhere in this function or in
`prepare_audio_for_transcription`'s use of it. `language.lower() == ...` is the
whole contract. The Python tests in `tests/test_media.py`
(`test_get_audio_track_by_language`, `..._case_insensitive`) only ever exercise
exact-string and case folding.

## EXACT golden (Python) vs Rust, same inputs

Tracks: `[index0=jpn, index1=eng, index2=und]`. Verified by running the Python
spec directly (`python3 -c "from submate.media import ..."`):

| query | Python golden (spec) | Rust (current) | match? |
|-------|----------------------|----------------|--------|
| `"ja"`  | `None`              | index 0        | **DIVERGE** |
| `"en"`  | `None`              | index 1        | **DIVERGE** |
| `"jpn"` | index 0             | index 0        | ok |
| `"und"` | index 2 (returned!) | `None`         | **DIVERGE** |
| `"JPN"` | index 0             | index 0        | ok |

Three divergent rows. Two distinct failure modes:

1. **Over-matching (`ja`, `en`):** Rust resolves a 639-1 query to a 639-2-tagged
   track; Python returns `None`. Downstream this is user-visible: in
   `prepare_audio_for_transcription`, a `None` match **falls back to
   `audio_tracks[0]`** (the first track), whereas Rust extracts the matched
   Japanese track instead. Different audio is fed to Whisper.

2. **Under-matching (`und`):** Python's pure string compare returns the `und`
   track for an exact `und` query. Rust's `track_language_matches` explicitly
   short-circuits `LanguageCode::None` (which `und`/unknown parse to) and
   returns `None`, so an exact `und` request can no longer select an
   `und`-tagged track.

The same normalization was applied to `resolve_audio_selector` and
`lang_match_is_ambiguous`, so `AudioSelector::Lang("ja")` against `jpn` tracks
now resolves+reports-ambiguous in Rust where the Python-equivalent string
match would find no match. The Rust test
`audio_selector_lang_picks_first_of_several_matches` was *edited in the same
commit* to assert the new normalized behavior (it previously asserted
`NoLanguageMatch` for `"ja"` vs `jpn`), so the parity suite now encodes the
wrong expectation and passes green.

## falsifier

A `parity::` test that pins `get_audio_track_by_language` /
`resolve_audio_selector` against the Python spec values above must exist and
pass. Concretely: with tracks `[jpn, eng, und]`,
`get_audio_track_by_language(tracks, "ja") == None`,
`... "en") == None`, `... "und") == Some(index2)`, `... "jpn") == Some(index0)`,
`... "JPN") == Some(index0)`. Today no such test exists; the present
`audio_track_language_normalizes` asserts the opposite for `ja`/`en`/`und`.

## fix direction (for the implementer)

Revert the matching back to case-folded raw-string equality to match the spec:

- `get_audio_track_by_language`: `track.language.to_lowercase() ==
  language.to_lowercase()` (the pre-`bb9b252` body).
- `resolve_audio_selector` `AudioSelector::Lang` arm and
  `lang_match_is_ambiguous`: same case-folded string compare, drop the
  `LanguageCode::from_string` / `track_language_matches` path.
- Delete/replace `track_language_matches`; restore
  `audio_selector_lang_picks_first_of_several_matches` to assert
  `NoLanguageMatch` for `"ja"` vs `jpn`-only tracks.
- Replace `audio_track_language_normalizes` with the spec-table assertions
  above (and the `und`-exact-match row).

If anime-dub 639-1→639-2 selection is genuinely desired, it is a **spec change
to `submate/media.py` first** (with matching Python tests + a regenerated
golden), not a Rust-only behavior. As of this round the Python spec does not
normalize, so the Rust port must not either.
