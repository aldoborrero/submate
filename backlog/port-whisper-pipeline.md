# End-to-end transcription pipeline

**blocked-by:** port-whisper-rs-wire, port-stablets-output-D, port-media-extract

## what
Wire media → PCM → whisper-rs → regroup → suppress_silence → to_srt_vtt into one pipeline entry point matching `WhisperModelWrapper.transcribe`.

## where
`rust/crates/submate-whisper/src/lib.rs`.

## why
The full transcription path the CLI + queue call.

## falsifies
`cargo test -p submate-whisper parity::transcribe` passes `parity::assert_segments_close` (count ±1, time ±200ms, text-ratio ≥0.9) against `rust/fixtures/transcribe/*.segments.json`. (Structural — whisper.cpp ≠ faster-whisper.)
