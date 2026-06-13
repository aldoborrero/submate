# Port whisper.py `_audio_description` safe-logging formatter

**blocked-by:** port-whisper-pipeline

## what
`WhisperModelWrapper._audio_description` (submate/whisper.py, lines 352-369)
formats a short, path-safe description of the audio input used in the
"Transcribing ..." log line. It deliberately avoids logging full file paths
(stated security concern) and groups byte counts with thousands separators.
This pure-data formatter is not ported.

Port it as a pure function mirroring the three branches exactly:

- bytes input  → `<bytes: {len:,} bytes>`   (e.g. `<bytes: 1,234 bytes>`)
- BytesIO/buffer → `<BytesIO: {len:,} bytes>`
- path/str input → `<file: {basename}>`      (filename only, never the dir)

The `{:,}` formatting is Python's thousands-grouping with a comma; Rust has no
built-in `,` grouping, so the implementer must format the integer with comma
separators every three digits (e.g. 1234 → `1,234`, 999 → `999`,
1000000 → `1,000,000`). The path branch must strip to the final component only
(`Path(audio).name`): `/media/movie.mkv` → `movie.mkv`, and a bare `audio.wav`
→ `audio.wav`.

Suggested signature mirroring the input union:
`pub fn audio_description(input: &AudioInput) -> String` where `AudioInput`
is `Bytes(usize) | BytesIo(usize) | File(&Path)` (the length, not the buffer,
is all the formatter needs) — keep it allocation-light and dependency-free.

## where
`rust/crates/submate-whisper/src/lib.rs`, plus its parity test
`rust/crates/submate-whisper/tests/parity.rs`.

## why
This string is operator-facing log output and a security control (paths are
intentionally truncated to the basename). If the Rust port leaks the full path
or formats byte counts differently, it both regresses the security intent and
diverges from the Python spec's observable behavior. Small, fully pure, and a
clean leaf — good first whisper-crate parity unit independent of inference.

## falsifies
`cargo test -p submate-whisper parity::audio_description` asserts byte-exact
equality against a golden table under
`rust/fixtures/transcribe/audio_description.json` — an array of
`{input_kind, value, expected}` rows captured from
`WhisperModelWrapper._audio_description`:

- `{kind:"bytes", value:1234, expected:"<bytes: 1,234 bytes>"}`
- `{kind:"bytes", value:999, expected:"<bytes: 999 bytes>"}`
- `{kind:"bytes", value:1000000, expected:"<bytes: 1,000,000 bytes>"}`
- `{kind:"bytesio", value:5000, expected:"<BytesIO: 5,000 bytes>"}`
- `{kind:"file", value:"/media/movies/The Movie.mkv",
   expected:"<file: The Movie.mkv>"}`
- `{kind:"file", value:"audio.wav", expected:"<file: audio.wav>"}`

The test reads each row, calls `audio_description`, and `assert_eq!`s the
`expected` string. The thousands-grouping rows are the discriminating cases.

requires fixture: rust/fixtures/transcribe/audio_description.json — capture by
calling `WhisperModelWrapper._audio_description` on each input (bytes of the
given length, BytesIO of the given length, a Path with the given string) and
recording the returned string. I cannot touch rust/fixtures/ (denylisted);
flag for human capture before the implementer starts.
