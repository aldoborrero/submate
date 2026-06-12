"""Capture SRT/VTT round-trip goldens for byte-parity.

Falsifier target: submate-subtitle parity::srt_roundtrip / parity::vtt_roundtrip.
Translation parses then re-emits these files (submate/translation.py:
srt.parse -> srt.compose for SRT, pysubs2 from_string -> to_string for VTT), so
the Rust port must reproduce the *re-serialized* bytes, not the raw input.

This emits, for each representative input:
  - subtitle/<name>.srt / .vtt           : the round-tripped golden (parse+compose)
The Rust test re-emits its own parse of the same logical content and compares
byte-for-byte against these goldens.

Pure-data capture: srt + pysubs2 import in the nix devshell, no media/credentials.
"""

from __future__ import annotations

import srt

from _common import write_text

# Representative SRT inputs covering multi-cue, multi-line text, and blank lines
# inside a cue. Keys become fixture file stems.
SRT_INPUTS = {
    "basic": (
        "1\n"
        "00:00:01,000 --> 00:00:04,000\n"
        "Hello, world!\n"
        "\n"
        "2\n"
        "00:00:05,500 --> 00:00:08,250\n"
        "Second line\n"
        "and a wrap.\n"
    ),
    "single": (
        "1\n00:00:00,000 --> 00:00:02,000\nOnly one cue.\n"
    ),
}

# VTT inputs go through pysubs2 (format_="vtt"). Header + dot-millisecond stamps.
VTT_INPUTS = {
    "basic": (
        "WEBVTT\n"
        "\n"
        "00:00:01.000 --> 00:00:04.000\n"
        "Hello, world!\n"
        "\n"
        "00:00:05.500 --> 00:00:08.250\n"
        "Second line\n"
        "and a wrap.\n"
    ),
}


def main() -> None:
    for name, content in SRT_INPUTS.items():
        subs = list(srt.parse(content))
        roundtrip = str(srt.compose(subs))
        write_text(f"subtitle/{name}.srt", roundtrip)

    import pysubs2

    for name, content in VTT_INPUTS.items():
        subs = pysubs2.SSAFile.from_string(content, format_="vtt")
        roundtrip = subs.to_string("vtt")
        write_text(f"subtitle/{name}.vtt", roundtrip)


if __name__ == "__main__":
    main()
