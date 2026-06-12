# Port ASS parsing preserving inline tags

## what
Hand-roll an ASS/SSA parser that preserves inline override tags (`{\...}`) and newline markers through parse→edit→serialize. Do NOT use a third-party crate unless it round-trips tags identically.

## where
`rust/crates/submate-subtitle/src/lib.rs`.

## why
The LLM-translation path must not drop ASS formatting tags.

## falsifies
`cargo test -p submate-subtitle parity::ass_tags` round-trips `rust/fixtures/subtitle/*.ass` preserving every `{\...}` tag.
