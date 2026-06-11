# Port audio extraction to PCM

**blocked-by:** port-media-probe

## what
Port `extract_audio_track_to_memory` / `prepare_audio_for_transcription` — ffmpeg `-map 0:a:N -f s16le -ac 1 -ar 16000 pipe:`.

## where
`rust/crates/submate-media/src/lib.rs`.

## why
Produces the PCM the whisper pipeline consumes.

## falsifies
`cargo test -p submate-media extract_pcm_sha` — extracted PCM sha256 equals `rust/fixtures/media/*.pcm.sha256`.
