"""Capture on-disk subtitle discovery + filename-language cases.

Falsifier target: submate-subtitle parity::discovery_fs. Lays out the
test_subtitle.py scenarios in a temp dir and dumps inputs -> outputs so the
Rust port reproduces get_external_subtitle_paths / parse_subtitle_language /
get_lrc_path byte-for-byte (set-equality on returned file names, exact
LanguageCode.value on each parse).

Runs against the Python submate to author the golden; a human/META capture
pre-pass runs it once and commits rust/fixtures/subtitle/discovery_cases.json.
"""

from __future__ import annotations

import tempfile
from pathlib import Path

from _common import write_json

from submate.subtitle import (
    get_external_subtitle_paths,
    get_lrc_path,
    parse_subtitle_language,
)

# Each scenario: a directory layout (files to touch) + the video filename whose
# stem drives discovery. The expectations are computed live from the Python
# implementation so the golden is always the source of truth.
SCENARIOS = [
    {
        "label": "prefix_collision",
        "video": "Episode 1.mkv",
        "files": [
            "Episode 1.mkv",
            "Episode 1.en.srt",
            "Episode 10.en.srt",
        ],
    },
    {
        "label": "reversed_scan_trailing_tag",
        "video": "movie.mkv",
        "files": [
            "movie.mkv",
            "movie.no.forced.en.srt",
        ],
    },
    {
        "label": "language_before_flag",
        "video": "movie.mkv",
        "files": [
            "movie.mkv",
            "movie.en.forced.srt",
        ],
    },
    {
        "label": "subgen_marker",
        "video": "movie.mkv",
        "files": [
            "movie.mkv",
            "movie.subgen.medium.en.srt",
        ],
    },
    {
        "label": "no_language_segment",
        "video": "movie.mkv",
        "files": [
            "movie.mkv",
            "movie.srt",
        ],
    },
    {
        "label": "mixed_extensions",
        "video": "movie.mkv",
        "files": [
            "movie.mkv",
            "movie.en.srt",
            "movie.es.vtt",
            "movie.ass",
            "movie.txt",  # not a subtitle extension
        ],
    },
]

# get_lrc_path cases (audio path -> .lrc sibling, pathlib with_suffix semantics)
LRC_CASES = [
    "song.mp3",
    "track.flac",
    "noext",
    "/media/audio/episode.m4a",
    "archive.tar.gz",
]


def main() -> None:
    out: dict[str, object] = {"discovery": {}, "lrc": {}}

    for scenario in SCENARIOS:
        with tempfile.TemporaryDirectory() as td:
            tmp = Path(td)
            for name in scenario["files"]:
                (tmp / name).write_text("", encoding="utf-8")
            video_path = tmp / scenario["video"]
            video_stem = video_path.stem

            external = get_external_subtitle_paths(video_path)
            # Set of file names only — scan order is not contractual (iterdir).
            expect_external = sorted(p.name for p in external)
            expect_parse = {
                p.name: parse_subtitle_language(p, video_stem).value for p in external
            }
            out["discovery"][scenario["label"]] = {
                "video_stem": video_stem,
                "dir_files": sorted(scenario["files"]),
                "expect_external": expect_external,
                "expect_parse": expect_parse,
            }

    for audio in LRC_CASES:
        out["lrc"][audio] = str(get_lrc_path(Path(audio)))

    write_json("subtitle/discovery_cases.json", out)


if __name__ == "__main__":
    main()
