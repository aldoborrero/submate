# Port Bazarr s16le PCM → f32 sample decode (whisper-rs input)

**blocked-by:** port-bazarr-pcm-wav-wrap

## what
In the Python tool, Bazarr's raw PCM is wrapped in a WAV header and handed to
stable-whisper / PyAV, which internally decodes s16le → float. In the Rust
topology `submate-whisper::transcribe_pcm` takes `Vec<f32>` directly (mono,
16 kHz, range `-1.0..=1.0`), so the WAV-file detour collapses into one pure
decode function that replaces it.

Port that decode as a pure, deterministic function:

- `pub fn pcm_s16le_to_f32(bytes: &[u8]) -> Vec<f32>`
  - Interpret `bytes` as little-endian `i16` samples (2 bytes each), mono.
  - Convert each sample to f32 by dividing by `32768.0`
    (`i16::MIN as f32 / 32768.0 == -1.0`; `i16::MAX` → `32767/32768`). This
    matches the standard s16→float convention; pin the exact divisor in the
    falsifier so a 32767-vs-32768 mistake fails.
  - A trailing odd byte (incomplete final sample) is dropped, mirroring
    `chunks_exact(2)`.
- If the input begins with `b"RIFF"`, strip the 44-byte canonical WAV header
  before decoding (reuse the wrap layout from `port-bazarr-pcm-wav-wrap` — the
  two are inverses on the canonical header). Raw PCM (no RIFF) decodes from
  offset 0. Keep header-skip limited to the canonical 44-byte layout this tool
  produces; do NOT pull in a general WAV parser (out of scope, separate item).

This is the byte-exact bridge from `wrap_pcm_as_wav`'s domain to
`submate-whisper`'s `Vec<f32>` input — the thing every Bazarr ASR job decodes
before inference.

## where
`rust/crates/submate-bazarr/src/lib.rs`, alongside `wrap_pcm_as_wav`.

## why
whisper-rs needs f32 samples; Bazarr sends s16le bytes. This is the only place
that conversion happens for the synchronous Bazarr path, and it is pure-data so
it must be byte-exact (token-set tolerance applies to transcription *output*,
never to the sample decode feeding it). A wrong scale factor or endianness
silently shifts every amplitude and degrades transcription.

## falsifies
`cargo test -p submate-bazarr parity::pcm_decode` asserts, within `1e-7`:

- `pcm_s16le_to_f32(<raw s16le golden bytes>)` ==
  `parity::load_f32("bazarr/pcm/sine440.f32")` via `parity::assert_f32_close`
  (existing helper, epsilon `1e-7`).
- `pcm_s16le_to_f32(wrap_pcm_as_wav(<raw s16le golden bytes>))` produces the
  same f32 vector (RIFF-prefixed input header-strips to the identical samples),
  proving the wrap/decode round-trip.

requires fixture: rust/fixtures/bazarr/pcm/sine440.pcm (shared with
port-bazarr-pcm-wav-wrap) AND rust/fixtures/bazarr/pcm/sine440.f32 — the
reference float samples (Python: read sine440.pcm as `np.frombuffer(dtype=i16)`
then `astype(f32) / 32768.0`, dump little-endian f32). Capture first via the
same `capture_bazarr_audio.py` that produces the wrap golden. I cannot touch
rust/fixtures/ (denylisted); flag for human capture before the implementer
starts.
