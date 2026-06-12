# cli-audio: `submate probe` command to list audio tracks

**blocked-by:** cli-audio-track-disposition

## what
Add a `submate probe <file>` subcommand that prints the file's audio tracks so a
user can see what is in a multi-track file before choosing one to transcribe.
For each audio track show: stream index, language tag, codec, channel/title
info, and a marker on the default track. Example:

```
3 audio tracks in movie.mkv:
  #0  jpn  ac3   Main          (default)
  #1  eng  aac   English Dub
  #2  jpn  ac3   Commentary
```

The rendering must be a **pure function** of `&[AudioTrack]` so it is unit
testable without invoking `ffprobe`; the subcommand is the thin I/O wrapper that
calls `get_audio_tracks(file).await` and prints the rendered string. The same
renderer will later back the interactive picker (cli-audio-interactive-picker).

## where
- `rust/crates/submate-cli/src/main.rs` — add `Probe(ProbeArgs)` to the
  `Command` enum and a `cmd_probe`; add `fn render_track_table(tracks: &[AudioTrack]) -> String`.
- Uses `submate_media::get_audio_tracks` and the `default`/`title` fields from
  cli-audio-track-disposition.

## why
You can't sensibly pass `--audio track:2` or `--audio ja` without first knowing
the file's layout — especially for untagged (`und`) or duplicate-language
tracks. The library already returns everything needed; only a command and a
renderer are missing.

## falsifies
`cargo test -p submate-cli` green, including `probe_table_renders_tracks` that
passes a fixed `Vec<AudioTrack>` (mixed languages, one `default=true`, one with
`title=None`, one `language="und"`) to `render_track_table` and asserts the
output lists each track's index/language/codec/title and marks exactly the
default track. (The `ffprobe` call itself is out of scope for the test.)
