# Port `submate transcribe` file-collection + extension-formatting (pure-data)

**blocked-by:** none (the only dependency — `is_video_file` / `is_audio_file` /
`VIDEO_EXTENSIONS` / `AUDIO_EXTENSIONS` — is already ported in
`rust/crates/submate-paths/src/lib.rs`).

## what

Port the **pure-data file-discovery classifier** that `submate transcribe` runs
on its `PATH` argument, plus the supported-extension display helper. This is the
decision layer of `submate/cli/commands/transcribe.py` — the part that decides
*which* files end up in `files_to_process`, which land in `skipped_files`, and
which are ignored entirely — carved out from the I/O, Rich-console, queueing,
and Jellyfin-refresh concerns of the broader `port-cli-commands` umbrella (whose
single falsifier is only `cli_help`, so this classifier is currently untested by
any parity check).

Two functions to port, both deterministic given a directory listing:

1. `format_supported_extensions(extensions: set[str]) -> str`
   (module-level in `transcribe.py`):
   - strip a leading `.` from each extension (`ext.lstrip(".")` — note
     `lstrip` strips *all* leading dots, not just one, so `..srt` → `srt`;
     mirror `lstrip`, not a single-char trim),
   - sort the resulting tokens lexicographically (Python `sorted`, default
     string ordering on the already-dot-stripped tokens),
   - join with `", "`.
   Used to render the "Supported video: ..." / "Supported audio: ..." hints.
   Port as `fn format_supported_extensions(extensions: &[&str]) -> String` (or
   over an iterator of `&str`); take the inputs as a set/slice and reproduce the
   strip→sort→`", "`-join exactly.

2. The directory-scan classifier — extract the body of the `path_obj.is_dir()`
   branch into a pure function over a *list of relative path strings* (so the
   port needs no real filesystem; the golden supplies the listing). For each
   entry that is a file:
   - if `is_video_file(name) || is_audio_file(name)` → **process** (goes to
     `files_to_process`);
   - else if the basename does **not** start with `"."` **and** the lowercased
     extension is **not** in the ignore set
     `{".txt", ".jpg", ".png", ".nfo", ".srt", ".vtt"}` → **skipped**
     (counted as a non-media file the user is told about);
   - otherwise (dotfile, or one of those 6 ignore extensions) → **ignored**
     (dropped silently, counted in neither bucket).
   Suggested signature:
   `fn classify_dir_entries(names: &[&str]) -> (Vec<String>, Vec<String>)`
   returning `(files_to_process, skipped_files)` in **input/iteration order**
   (Python builds both lists by appending while iterating `glob`; the golden
   fixes a deterministic ordering — preserve it, do not sort either bucket).

   Contract notes that MUST match Python byte-for-byte:
   - extension matching for the ignore set is **case-insensitive on the
     extension** (`file.suffix.lower()`), while the dotfile guard is on the raw
     basename (`file.name.startswith(".")`) — so `.HIDDEN.TXT` is ignored via
     the dotfile rule, and `clip.SRT` is ignored via the lowercased-ext rule.
   - the video/audio test wins first: a file whose ext is in BOTH the media sets
     and (hypothetically) the ignore set would still be **processed** — preserve
     the `if … elif …` precedence.
   - `is_video_file` / `is_audio_file` already lowercase the extension in the
     paths crate; reuse them, do not re-implement extension matching.

   The recursive-vs-flat glob (`"**/*"` vs `"*"`), the `> 100` confirm prompt,
   the single-file branch, and all Rich output are **out of scope** — they are
   I/O / interaction and belong to `port-cli-commands`. This item is only the
   in-memory classifier + the formatter, so it stays one-worktree-sized.

## where

`rust/crates/submate-cli/src/transcribe_collect.rs` (new module, sibling of the
existing `translate_paths.rs` and `config_show.rs` pure-data modules), wired
into `main.rs` with the same `mod transcribe_collect;` stub-comment convention
those two use until `port-cli-commands` consumes them. Depends only on
`submate-paths`.

## why

This is the part of `submate transcribe` that determines the *work set* — get
the classification wrong and the Rust CLI either transcribes junk files or
silently drops real media, diverging from Python before a single byte of audio
is decoded. It is pure data (list of names in → two ordered lists out), so it is
exactly parity-testable and must match the Python SPEC exactly, like the
sibling `translate_paths` / `config_show` renderers already do.

## falsifies

`cargo test -p submate-cli transcribe_collect` — a `mod tests` (named off the
`parity::` path is acceptable here only if it matches the sibling convention;
prefer `transcribe_collect::tests::*` mirroring `config_show::tests::*`, and see
`backlog/parity-submate-cli-module-name.md` for why these in-crate tests must
still be run by the documented falsifier command) that:

- table-drives every case in
  **`rust/fixtures/cli/transcribe_collect_cases.json`** — each case a
  `{ "names": [...], "files_to_process": [...], "skipped_files": [...] }`
  triple — and asserts `classify_dir_entries(names)` equals the two golden
  lists **in order** via `parity::assert_json_eq` (or direct `assert_eq!` on
  the tuple serialized to the same shape);
- asserts `format_supported_extensions` over
  `submate_paths::VIDEO_EXTENSIONS` and `AUDIO_EXTENSIONS` equals the
  golden strings under a
  **`rust/fixtures/cli/transcribe_supported_extensions.json`**
  (`{ "video": "...", "audio": "..." }`) so the dot-strip + sort + join is
  pinned against the real extension sets.

**requires fixtures: `rust/fixtures/cli/transcribe_collect_cases.json` and
`rust/fixtures/cli/transcribe_supported_extensions.json` (capture first).**
Neither exists yet and the porter cannot create them (`rust/fixtures/` is
denylisted). Capture via a new
`rust/fixtures/capture/capture_cli_transcribe.py` (sibling of
`capture_cli_translate.py`, registered in `run_deterministic.sh`) that:
- imports `format_supported_extensions` from
  `submate.cli.commands.transcribe` and `VIDEO_EXTENSIONS` / `AUDIO_EXTENSIONS`
  from `submate.paths`, and emits the `{ "video", "audio" }` golden;
- replays the `is_dir()` classifier body over a fixed list of synthetic
  basenames covering: a clear video (`movie.mkv`), audio (`song.flac`), a
  dotfile (`.hidden.mkv`), each of the 6 ignore extensions
  (`note.txt`, `cover.jpg`, `poster.png`, `movie.nfo`, `subs.srt`, `cap.vtt`),
  a mixed-case ignore ext (`subs.SRT`), an unknown ext that becomes *skipped*
  (`archive.zip`), and a dotfile whose ext would otherwise be skipped
  (`.archive.zip`) — to pin every branch of the dotfile / lowercased-ext /
  media-wins-first precedence. Build the listing in a deterministic order and
  record the resulting `files_to_process` / `skipped_files` exactly as the
  Python branch produces them.
