# cli-audio: normalize language codes when matching audio tracks

## what
The `-a <lang>` / `lang:<code>` selector matches the audio track's language tag
**literally** (lowercased string equality), so a two-letter ISO 639-1 code does
not match a three-letter ISO 639-2 tag. MKV/MP4 containers almost always tag
audio with **639-2** (`jpn`, `eng`, `ger`), while users type — and `--help`
advertises — **639-1** (`ja`, `en`, `de`). Result: `-a ja` on a `jpn`-tagged
file fails with `no audio track for language 'ja'; available: jpn` and falls
back to the default track. This breaks the headline use case (picking the
Japanese dub on an anime MKV).

Fix: normalize both the requested selector code and each track's tag through the
existing `submate-lang` crate before comparing, so `ja`↔`jpn`, `en`↔`eng`,
native/English names, and case all resolve to the same `LanguageCode`.

`submate_lang::LanguageCode::from_string(Some(&str))` already accepts 639-1,
639-2 (T and B), and language names, mapping each to one canonical enum
(`und`/unknown → `LanguageCode::None`). Compare on that enum instead of raw
strings; treat two `None`s as a non-match (an untagged track must not match an
arbitrary requested code).

## where
- `rust/crates/submate-media/src/lib.rs` — `get_audio_track_by_language` (and
  the `resolve_audio_selector` `Lang` arm that calls it): replace the
  `to_lowercase()` string compare with
  `LanguageCode::from_string(Some(&track.language)) == LanguageCode::from_string(Some(requested))`,
  rejecting the `None == None` case.
- Add a `submate-lang` dependency to `submate-media` if not already present.

## why
This is the difference between `-a ja` working and not working on real files.
The conversion table already exists and is the single source of truth for the
113-language set; the selector should not reinvent (or skip) it.

## falsifies
`cargo test -p submate-media` green, including `audio_track_language_normalizes`:
given tracks tagged `jpn` and `eng`, `get_audio_track_by_language(.., "ja")`
returns the `jpn` track and `get_audio_track_by_language(.., "en")` returns the
`eng` track; `get_audio_track_by_language(.., "jpn")` still works; an untagged
(`und`) track is not returned for any specific requested code; and a request
with no matching language returns `None`.
