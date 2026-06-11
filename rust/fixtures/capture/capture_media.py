"""Capture ffprobe/extract goldens for one media clip.

Usage (inside the devshell, ffmpeg on PATH):
    python capture_media.py /path/to/clip.mkv

Falsifier targets: submate-media parity::probe, submate-media extract_pcm_sha.

Emits:
  media/<stem>.probe.json   — audio tracks (index/language/codec)
  media/<stem>.pcm.sha256   — sha256 of the extracted s16le/mono/16kHz PCM
"""

from __future__ import annotations

import dataclasses
import hashlib
import sys
from pathlib import Path

from _common import write_json, write_text

from submate import media


def main() -> None:
    if len(sys.argv) != 2:
        print(__doc__)
        sys.exit(2)
    clip = Path(sys.argv[1])
    stem = clip.stem

    tracks = media.get_audio_tracks(str(clip))
    write_json(
        f"media/{stem}.probe.json",
        [dataclasses.asdict(t) if dataclasses.is_dataclass(t) else vars(t) for t in tracks],
    )

    # Extract the first track to PCM and hash it (deterministic given ffmpeg).
    first_index = tracks[0].index if tracks else 0
    pcm = media.extract_audio_track_to_memory(str(clip), first_index, "s16le")
    data = pcm.getvalue() if hasattr(pcm, "getvalue") else bytes(pcm)
    digest = hashlib.sha256(data).hexdigest()
    write_text(f"media/{stem}.pcm.sha256", digest + "\n")
    print(f"{stem}: {len(tracks)} tracks, pcm sha256={digest[:16]}…")


if __name__ == "__main__":
    main()
