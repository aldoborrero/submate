# Port ffprobe audio-track probing

## what
Port `get_audio_tracks` / `get_audio_languages` from `submate/media.py` — run `ffprobe -show_streams -select_streams a -of json` and parse index/language/codec.

## where
`rust/crates/submate-media/src/lib.rs`. Use `tokio::process` + `serde_json`.

## why
The queue picks an audio track by language before transcription.

## falsifies
`cargo test -p submate-media parity::probe` parses a sample ffprobe JSON payload
(an inline `&str` const inside the test module — NOT a file under
`rust/fixtures/media/`) and asserts the extracted index/language/codec match.

## scope (re-scoped 2026-06-12 to avoid denylist)
Do NOT author or modify anything under `rust/fixtures/`. The prior attempt
(`grind/port-media-probe`, see `backlog/tried/port-media-probe.md`) was rejected
solely for creating `rust/fixtures/media/sample.probe.json`, a denylisted golden
fixture. The falsifier above needs no such fixture: embed the representative
`ffprobe -show_streams -select_streams a -of json` output as an inline string
constant in the test and assert parsing against it. (An additional opt-in test
may invoke the real `ffprobe` binary when present, but it must be skipped when
absent and must not write fixtures.) Allowed scope is limited to
`rust/crates/submate-media/`.
