# Port paths.py to submate-paths

**blocked-by:** port-types-enums, port-lang-enum

## what
Port subtitle-path building and the Docker path-mapping translation from `submate/paths.py` (build_subtitle_path, map_path, is_video_file/is_audio_file, extension sets).

## where
`rust/crates/submate-paths/src/lib.rs`. Use `camino` for UTF-8 paths.

## why
Pure string/path logic the queue + server rely on; exact-match testable.

## falsifies
`cargo test -p submate-paths parity::path_cases` passes exact-match against `rust/fixtures/paths/path_cases.json`.
