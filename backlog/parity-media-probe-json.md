# parity-media: probe.json golden falsifier for get_audio_tracks

## what
`submate-media` has a parity test for PCM extraction (`extract_pcm_sha`, gated
on the not-yet-captured `media/clipA.pcm.sha256`) but **no parity test asserts
the audio-track *probe* output** — `parse_audio_tracks` / `get_audio_tracks` —
against the `media/<stem>.probe.json` golden that `capture_media.py` is built to
emit. The probe path is the half of `submate/media.py` that produces structured
data (`index`/`language`/`codec`), and it is currently checked only against an
inline `SAMPLE_PROBE_JSON` string hand-written in the Rust test module, never
against the Python golden.

`rust/fixtures/capture/capture_media.py` already writes
`media/<stem>.probe.json` as
`[dataclasses.asdict(t) for t in media.get_audio_tracks(clip)]`, i.e. a JSON
array of `{"index", "language", "codec"}` objects — exactly the Python
`AudioTrack` dataclass fields. There is no Rust test that loads that file and
compares.

## shape divergence the test must account for
The Rust `AudioTrack` (`rust/crates/submate-media/src/lib.rs`) carries **two
extra fields the Python golden does not have**:

- `default: bool`   (from ffprobe `disposition.default == 1`)
- `title: Option<String>`  (from `tags.title`)

These are a deliberate Rust-only extension for the typed `-a` selector
(commits `b66196b`, `c208ebd`, `5b082a5`). The Python `AudioTrack` dataclass and
therefore `media/<stem>.probe.json` contain ONLY `{index, language, codec}`. A
naive `assert_json_eq` of a serialized Rust `AudioTrack` against the golden
would FAIL on the extra `default`/`title` keys.

The falsifier must compare on the **Python-visible subset** — project each Rust
`AudioTrack` to `{index, language, codec}` (in that key order) and diff that
against the golden array. This pins the ported fields exactly while leaving the
Rust-only fields out of the parity contract (they have no Python truth to match).

## where
- `rust/crates/submate-media/src/lib.rs` — add a `parity::probe_json` test next
  to the existing `extract_pcm_sha` module.
- Golden: `rust/fixtures/media/<stem>.probe.json` (does not exist yet — produced
  by a manual `rust/fixtures/capture/capture_media.py <clip>` run against a
  multi-track clip; `media/` is merge-denylisted like the other goldens, so the
  test must self-skip when the fixture is absent, mirroring `extract_pcm_sha`).

## why
The probe golden is exact-diffable pure data (no ffmpeg nondeterminism in the
JSON, unlike the PCM bytes), so it is the higher-confidence half of the media
contract and currently has zero golden coverage. Without it, a regression in
the `und`/`unknown` defaults, the index re-enumeration (ffprobe `index` → 0-based
`AudioTrack.index`), or the `tags.language` lookup would pass CI because only an
inline sample string guards it.

## falsifies
`cargo test -p submate-media parity::probe_json` exists and passes against
`rust/fixtures/media/<stem>.probe.json`: each Rust `AudioTrack` projected to
`{index, language, codec}` equals the corresponding golden object, array length
matches, and the test self-skips (no-op pass) when the golden has not been
captured — so it arms itself the moment the fixture lands.

## prerequisite (fixture capture, not a port task)
Run `rust/fixtures/capture/capture_media.py` against a representative
multi-track clip to emit `media/<stem>.probe.json` + `media/<stem>.pcm.sha256`.
This is a deliberate capture run (Python tree, ffmpeg on PATH), not grind work;
it also arms the existing `extract_pcm_sha` falsifier.
