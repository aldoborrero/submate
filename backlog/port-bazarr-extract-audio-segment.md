# Port Bazarr detect-language audio-segment trim (ffmpeg atrim)

**blocked-by:** port-bazarr-pcm-wav-wrap

## what

Port `extract_audio_segment` from `submate/server/handlers/bazarr/audio.py`:
the `detect-language` route's pre-step that trims an `offset..offset+length`
second window out of Bazarr's uploaded audio and re-encodes it to the canonical
`s16le` / mono / 16 kHz PCM the detection task consumes. It is the ONE piece of
the Bazarr detect-language flow no open item covers:

- `port-bazarr-pcm-wav-wrap` / `port-bazarr-pcm-to-f32` are header-wrap and
  sample-decode — neither trims a time window.
- `port-server-bazarr-asr` wires the routes and relays the body.
- `align-bazarr-route-signatures` pins the query params
  (`detect_lang_offset`, `detect_lang_length` with `ge`/`le` bounds) and the
  `LanguageDetectionResponse` shape — but explicitly NOT the trim itself.

Spec (Python `ffmpeg-python` graph, all flags wire-exact):

```
ffmpeg.input("pipe:", format="wav")
      .filter("atrim", start=offset, duration=length)
      .output("pipe:", format="s16le", acodec="pcm_s16le", ac=1, ar=16000)
```

so the equivalent CLI the Rust port spawns is:

```
ffmpeg -f wav -i pipe: -af atrim=start=<offset>:duration=<length> \
       -f s16le -acodec pcm_s16le -ac 1 -ar 16000 pipe:
```

Signature target (a deterministic ffmpeg subprocess wrapper, mirroring
`submate-media::extract_audio_track_to_memory`'s pattern — stdin WAV bytes in,
captured stdout PCM out, non-zero exit → error carrying stderr):

```rust
pub async fn extract_audio_segment(
    wav_bytes: &[u8],
    offset_secs: u32,   // Python default 0  (route bound: ge=0)
    length_secs: u32,   // Python default 30 (route bound: ge=1, le=300)
) -> Result<Vec<u8>, SegmentError>;
```

Faithfulness constraints:

- Input is WAV (the route hands `uploaded_audio`'s buffer, format `"wav"`); do
  NOT feed raw headerless PCM — the `-f wav` demux is part of the contract.
- `atrim start=offset duration=length` is an audio filter (`-af`), not `-ss`/`-t`
  seek; keep it a filter so the sample-accurate frame boundaries match Python's
  graph (a `-ss`-based seek can land on a different frame and drift the window).
- Output flags fixed: `s16le` / `pcm_s16le` / `ac=1` / `ar=16000` (reuse the
  `PCM_FORMAT` constant convention from `submate-media`).
- Errors: ffmpeg spawn failure or non-zero exit → `SegmentError` carrying the
  captured stderr (mirrors `submate-media::ExtractError`). The Python wrapper
  raises `RuntimeError(f"Audio segment extraction failed: {e}")`; the route's
  `except Exception` then swallows it into the 200 `"Unknown"/"und"` response
  (that swallow lives in the route, pinned by `align-bazarr-route-signatures` —
  out of scope here; this item only owns the trim function and its error type).

## where

`rust/crates/submate-bazarr/src/lib.rs`, alongside `wrap_pcm_as_wav` /
`pcm_s16le_to_f32`. The `detect-language` route (`submate-server`, under
`port-server-bazarr-asr`) calls it before enqueueing the detection job.

## why

Bazarr's auto-detect posts a whole audio file and asks the provider to sample a
window (default first 30 s) for language ID. If the window is wrong — offset
ignored, duration unbounded, wrong sample rate — the detection task either reads
the wrong audio or chokes on an unexpected format, and Bazarr's auto-detect
silently mislabels every file. The trim is pure-data given fixed input bytes:
same WAV + same offset/length must yield byte-identical PCM, so it is
golden-testable, not token-tolerance.

## falsifies

`cargo test -p submate-bazarr parity::extract_segment` (ffmpeg-gated, skips with
an `eprintln` when `ffmpeg` is absent, same pattern as
`submate-media`'s `probes_a_generated_audio_file`):

1. `extract_audio_segment(<sine440.wav golden>, 0, 1)` == the golden PCM for a
   1-second trim of `sine440.wav`, compared byte-for-byte (or, if ffmpeg build
   skew makes raw bytes brittle across hosts, decode both via `pcm_s16le_to_f32`
   and compare with `parity::assert_f32_close` at `1e-4` AND assert the sample
   count equals `1 * 16000` mono samples — pin BOTH so an off-by-a-frame window
   or a wrong sample rate fails).
2. `extract_audio_segment(<sine440.wav golden>, offset, length)` with a non-zero
   offset yields a window whose leading samples differ from the offset-0 window
   (proves `start=offset` is honored, not dropped).

requires fixture: rust/fixtures/bazarr/pcm/sine440_seg_off0_len1.pcm (and, for
case 2, rust/fixtures/bazarr/pcm/sine440_seg_off1_len1.pcm) — capture by running
the exact ffmpeg graph above over the existing `rust/fixtures/bazarr/pcm/sine440.wav`
golden and dumping stdout. Add this to the same `capture_bazarr_audio.py`
prepass that produces `sine440.pcm`/`.wav`/`.f32`. I cannot touch rust/fixtures/
(denylisted) — flag for human capture before the implementer starts.
