# cli-audio: decouple whisper decode-language from track selection

**blocked-by:** cli-audio-selector-grammar

## what
Stop overloading one value to mean both "which audio track" and "what language
whisper should decode". Add a separate `-l`/`--language` flag that is **only**
the whisper decode hint, accepting an ISO code or `auto` (let whisper detect).

Behavior after this change:
- `--audio` selects the *track* (via `AudioSelector`); it no longer sets the
  decode language.
- `--language <code>` sets the decode hint explicitly.
- When `--language` is omitted, default the decode hint to the **selected
  track's language tag**; if that track is untagged (`und`/unknown), default to
  `auto` (None → whisper auto-detects).

Concretely: today `args.audio_language` is copied into BOTH `JobOpts.source_language`
(→ `TranscribeOptions.language`, the decode hint) and `AudioSource::File.language`
(→ track selection). Split these: the job carries the resolved `AudioSelector`
for track selection and a separately-resolved decode language for
`TranscribeOptions.language`.

## where
- `rust/crates/submate-cli/src/main.rs` — add `--language`/`-l` to
  `TranscribeArgs`; in `transcribe_files`, resolve decode-language independently
  of the selector and set `JobOpts.source_language` / the `AudioSource` selector
  accordingly.
- `rust/crates/submate-node/src/lib.rs` — where `opts.source_language` becomes
  `TranscribeOptions { language, .. }` (around the `let language = opts.source_language.clone()`
  site), ensure it now receives the decode language, not the track selector.

## why
A user dubbing-hunting wants "transcribe the Japanese track" — but may want
whisper to auto-detect, or to force a specific decode language that differs from
the tag. Today `-a ja` silently forces decode=ja too; you can't pick the JA
track and let whisper detect, or pick `track:2` (untagged) and hint `en`.

## falsifies
`cargo test -p submate-cli` green, including `decode_language_resolution_*`:
- `--audio track:2 --language en` → selector `Index(2)`, decode language `Some("en")`.
- `--audio ja` (no `--language`) → decode language defaults to `Some("ja")`.
- `--audio track:1 --language auto` → decode language `None` (auto-detect).
- selecting an untagged track with no `--language` → decode language `None`.

Assert against the resolved (selector, decode-language) pair the CLI builds for
the job — selector and decode hint must vary independently.
