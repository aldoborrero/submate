# cli-audio: capture default-disposition + title on AudioTrack

## what
Extend the media-layer audio-track model so a track knows whether it is the
container's **default** audio stream and carries its **title** tag. These two
fields are prerequisites for `submate probe` (a useful listing), the `default`
audio selector, and the smart-default rule (pick the default-flagged track when
several exist).

Today `AudioTrack` is `{ index, language, codec }` and `RawStream` only reads
`codec_name` + `tags`. `ffprobe -show_streams` already emits, per stream:

```json
{ "codec_name": "ac3",
  "disposition": { "default": 1, "comment": 0 },
  "tags": { "language": "jpn", "title": "Commentary" } }
```

Add:
- `AudioTrack.default: bool`  — from `disposition.default == 1`.
- `AudioTrack.title: Option<String>` — from `tags.title` (absent → `None`).

This is an **additive, Rust-only extension** — the Python `get_audio_tracks`
returns only language/codec, so there is no parity golden to match and existing
`parse_audio_tracks` parity behavior must stay unchanged for the existing
fields.

## where
- `rust/crates/submate-media/src/lib.rs` — `AudioTrack`, `RawStream` (add a
  `disposition` sub-struct with `#[serde(default)] default: u8`), `StreamTags`
  (add `title: Option<String>`), and the `parse_audio_tracks` mapping.

## why
Selecting "the track every player would pick" requires the default disposition,
and a human inspecting tracks needs the title to tell a dub from a commentary
track. Both come free from ffprobe; we just aren't reading them.

## falsifies
`cargo test -p submate-media` green, including a new test
`parse_audio_tracks_reads_disposition_and_title` that feeds a two-stream ffprobe
JSON (one with `disposition.default=1` + `tags.title="Main"`, one with
`disposition.default=0`, no title) and asserts the resulting `AudioTrack`s carry
`default=true/false` and `title=Some("Main")/None` respectively, while `index`,
`language`, and `codec` keep their existing values. Existing
`parse_audio_tracks` tests stay green (additive change).
