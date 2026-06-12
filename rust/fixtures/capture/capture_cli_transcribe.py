"""Capture `submate transcribe` collection helpers -> fixtures/cli/.

Falsifier target: submate-cli transcribe_collect. Drives the two deterministic
parts of the `submate transcribe` file-discovery layer from
submate/cli/commands/transcribe.py over a golden case table:

- format_supported_extensions(extensions)   -> str  (module-level helper)
- the `path_obj.is_dir()` classifier body, extracted here as a pure function
  over a list of basenames -> (files_to_process, skipped_files)

The classifier mirrors the `is_dir()` glob loop exactly: media files (video or
audio) are processed; non-dotfiles whose lowercased extension is not in the
6-element ignore set are skipped; everything else (dotfiles, ignore-ext files)
is dropped silently. The `if … elif …` precedence (media test wins first) and
the input/iteration order of both lists are preserved.
"""

from __future__ import annotations

from pathlib import Path

from _common import write_json

from submate.cli.commands.transcribe import format_supported_extensions
from submate.paths import AUDIO_EXTENSIONS, VIDEO_EXTENSIONS, is_audio_file, is_video_file

# The ignore set is duplicated from transcribe.py's is_dir() branch verbatim;
# keep these in sync if the Python spec changes.
_IGNORE_EXTENSIONS = {".txt", ".jpg", ".png", ".nfo", ".srt", ".vtt"}


def classify_dir_entries(names: list[str]) -> tuple[list[str], list[str]]:
    """Reproduce the file-classification branch of `transcribe`'s is_dir() loop.

    Operates on basenames (the golden supplies the directory listing) so no real
    filesystem is needed. Returns (files_to_process, skipped_files) in iteration
    order, matching Python's append-while-iterating behaviour.
    """
    files_to_process: list[str] = []
    skipped_files: list[str] = []
    for name in names:
        file = Path(name)
        if is_video_file(file) or is_audio_file(file):
            files_to_process.append(name)
        elif not file.name.startswith(".") and file.suffix.lower() not in _IGNORE_EXTENSIONS:
            skipped_files.append(name)
    return files_to_process, skipped_files


# Directory listings, each exercising a distinct branch of the classifier.
# Order is fixed so the golden output is deterministic.
CASES: list[list[str]] = [
    [
        "movie.mkv",  # video -> process
        "song.flac",  # audio -> process
        ".hidden.mkv",  # dotfile (media ext) -> ignored via dotfile rule
        "note.txt",  # ignore ext -> ignored
        "cover.jpg",  # ignore ext -> ignored
        "poster.png",  # ignore ext -> ignored
        "movie.nfo",  # ignore ext -> ignored
        "subs.srt",  # ignore ext -> ignored
        "cap.vtt",  # ignore ext -> ignored
        "subs.SRT",  # mixed-case ignore ext -> ignored via lowercased-ext rule
        "archive.zip",  # unknown ext -> skipped
        ".archive.zip",  # dotfile whose ext would otherwise skip -> ignored
    ],
    [],  # empty listing -> two empty lists
    [
        "a.mp4",  # ordering: interleaved media/skip preserves input order
        "b.zip",
        "c.wav",
        "d.iso",
    ],
]


def main() -> None:
    rows = []
    for names in CASES:
        files_to_process, skipped_files = classify_dir_entries(names)
        rows.append(
            {
                "names": names,
                "files_to_process": files_to_process,
                "skipped_files": skipped_files,
            }
        )
    write_json("cli/transcribe_collect_cases.json", rows)

    write_json(
        "cli/transcribe_supported_extensions.json",
        {
            "video": format_supported_extensions(VIDEO_EXTENSIONS),
            "audio": format_supported_extensions(AUDIO_EXTENSIONS),
        },
    )


if __name__ == "__main__":
    main()
