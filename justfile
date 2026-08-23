# submate task runner.
#
# Transcription needs the `model` feature (the whisper.cpp pipeline) plus
# `--sync` to process a file in one shot without a separate node. These recipes
# hide that mouthful behind `just transcribe …`.

manifest := "Cargo.toml"

# List recipes (default target).
default:
    @just --list

# Transcribe a media file in one shot, building with the whisper model feature.
# Extra flags are forwarded to the CLI, e.g. `just transcribe movie.mkv -t es`.
transcribe file *args:
    cargo run --manifest-path {{manifest}} -p submate-cli --features model -- \
        transcribe --sync {{file}} {{args}}

# Short alias for `transcribe`.
alias t := transcribe
