# UX: clean result summary + `just transcribe` wrapper

## what
On transcribe success, replace the bare "Processed: x -> y" with a clean summary,
e.g. `✓ movie.mkv → movie.srt (42 cues)`, deriving the cue count from the written
SRT. Add a `just transcribe <file>` recipe (and a short alias) so users avoid the
long `cargo run -p submate-cli --features model -- transcribe --sync …`.

## where
`rust/crates/submate-cli/src/main.rs` (summary formatter) + `justfile` (recipe
wrapping the model-feature build/run).

## why
Current output is noisy and the invocation is a mouthful — both hurt first-run UX.

## falsifies
`cargo test -p submate-cli result_summary_format` — the formatter yields the
expected string for a known (path, output_path, cue_count); and `just --summary`
lists a `transcribe` recipe.
