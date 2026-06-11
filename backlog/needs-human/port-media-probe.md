# Port ffprobe audio-track probing

## what
Port `get_audio_tracks` / `get_audio_languages` from `submate/media.py` — run `ffprobe -show_streams -select_streams a -of json` and parse index/language/codec.

## where
`rust/crates/submate-media/src/lib.rs`. Use `tokio::process` + `serde_json`.

## why
The queue picks an audio track by language before transcription.

## falsifies
`cargo test -p submate-media parity::probe` matches `rust/fixtures/media/*.probe.json` (mocked ffprobe JSON, or the real binary if present).
