# Port Bazarr raw-PCM → WAV-header wrapping

**META capture pre-pass LANDED (round 3 cleanup):** the goldens
`rust/fixtures/bazarr/pcm/sine440.{pcm,wav}` are committed and reproducible —
`capture_bazarr_audio.py` re-emits byte-identical output. The oracle exists;
the porter diffs against it and must NOT touch `rust/fixtures/**`. Item returned
to `backlog/` for automated pickup (was wrongly re-parked to `needs-human/`).

**blocked-by:** none (pure byte logic; no enum/lang/config deps)

## what
Port `WhisperModelWrapper._save_audio_with_wav_headers` from
`submate/whisper.py` (and its `_prepare_audio` dispatch) as a pure,
byte-for-byte function in Rust.

Two branches, both deterministic and content-only (drop the tempfile/cleanup
machinery — that is Python's PyAV-needs-a-path detour, not part of the data
contract):

1. **RIFF passthrough** — if the input begins with the 4 bytes `b"RIFF"`
   (`pcm_data[:4] == b"RIFF"`), return the bytes unchanged. Already a WAV
   container.
2. **Raw-PCM wrap** — otherwise treat the input as Bazarr's format (s16le =
   signed 16-bit little-endian, mono, 16 kHz) and prepend a canonical 44-byte
   WAV/RIFF header, exactly as Python's `wave.open(...).writeframes(pcm_data)`
   emits:
   - `b"RIFF"`, then `u32 LE` = `36 + data_len`, then `b"WAVE"`.
   - `b"fmt "`, `u32 LE` = 16, `u16 LE` audio_format = 1 (PCM),
     `u16 LE` channels = 1, `u32 LE` sample_rate = 16000,
     `u32 LE` byte_rate = `16000 * 1 * 2` = 32000,
     `u16 LE` block_align = `1 * 2` = 2, `u16 LE` bits_per_sample = 16.
   - `b"data"`, `u32 LE` = `data_len`, then the raw PCM bytes verbatim.

   Note: Python's `wave` module writes the size fields as it `close()`s after
   `writeframes`; for a single write the result is the standard 44-byte header
   followed by the PCM payload. Match those header bytes exactly (the falsifier
   pins them, so any off-by-one in byte_rate/block_align fails).

Signature suggestion: `pub fn wrap_pcm_as_wav(pcm: &[u8]) -> Vec<u8>`.

## where
`rust/crates/submate-bazarr/src/lib.rs` (currently a stub). This is the leaf
byte utility the Bazarr ASR ingestion path (`port-server-bazarr-asr`,
`port-queue-bazarr-service`) and `port-bazarr-pcm-to-f32` build on; keep it
free of HTTP/queue deps so it stays one-worktree-sized.

## why
Bazarr posts raw s16le PCM with no container. Every downstream decoder (PyAV
in Python, the f32 decode in the Rust topology) assumes a parseable WAV. This
is the boundary normalization; getting the header bytes wrong silently
corrupts every Bazarr transcription. Pure-data layer ⇒ byte-for-byte parity.

## falsifies
`cargo test -p submate-bazarr parity::wav_wrap` asserts, byte-for-byte:

- `wrap_pcm_as_wav(<raw s16le PCM golden>)` ==
  `rust/fixtures/bazarr/pcm/sine440.wav` (the Python `wave`-module output for
  the same PCM input), via `parity::assert_bytes_eq` (add the helper if absent;
  it is just `assert_eq!` on `&[u8]`).
- `wrap_pcm_as_wav(<bytes already starting with b"RIFF">)` returns the input
  unchanged (RIFF-passthrough branch), asserted against the same WAV golden fed
  back in.

requires fixture: rust/fixtures/bazarr/pcm/sine440.pcm (raw s16le mono 16 kHz,
e.g. a short 440 Hz tone) AND rust/fixtures/bazarr/pcm/sine440.wav (capture
first — run `submate`'s `_save_audio_with_wav_headers` on `sine440.pcm` and
dump the resulting file bytes). I cannot touch rust/fixtures/ (denylisted); a
`capture_bazarr_audio.py` under rust/fixtures/capture/ should generate both
from a deterministic numpy sine so the golden is reproducible. Flag for human
capture before the implementer starts.
