"""Capture `submate translate` filename/language helpers -> fixtures/cli/translate_filename_cases.json.

Falsifier target: submate-cli translate_paths. Drives the three deterministic
helpers from submate/cli/commands/translate.py over a golden case table:

- is_subtitle_file(path)               -> bool
- detect_source_language(file, source) -> str
- _output_path(file, target)           -> str  (the inline loop body, extracted)

_output_path mirrors the non-`--output` branch of the translate command loop
exactly (strip any trailing dotted stem segment, append .<target>, keep suffix).
"""

from __future__ import annotations

from pathlib import Path

from _common import write_json

from submate.cli.commands.translate import detect_source_language, is_subtitle_file


def _output_path(file: Path, target_lang: str) -> str:
    """Reproduce the inline default-output derivation in `translate`'s loop body."""
    stem = file.stem
    if "." in stem:
        base = stem.rsplit(".", 1)[0]
    else:
        base = stem
    return str(file.parent / f"{base}.{target_lang}{file.suffix}")


# (file, source_lang, target_lang) — covers every documented branch.
CASES = [
    ("movie.fr.srt", "auto", "es"),       # detect: fr (valid language tag)
    ("movie.v2.srt", "auto", "es"),       # detect: en (non-language token rejected)
    ("episode.01.srt", "auto", "es"),     # detect: en (numeric token rejected)
    ("movie.fr.srt", "de", "es"),         # detect: de (explicit source wins)
    ("movie.SRT", "auto", "es"),          # is_subtitle: true (case-fold)
    ("movie.tar.gz", "auto", "es"),       # is_subtitle: false (suffix .gz)
    ("movie.srt", "auto", "es"),          # output: movie.es.srt (no existing tag)
    ("movie.en.srt", "auto", "es"),       # output: movie.es.srt (.en replaced)
    ("movie.v2.srt", "auto", "es"),       # output: movie.es.srt (any trailing seg stripped)
    ("/media/movies/show.s01e01.fr.vtt", "auto", "en"),  # nested dir + .vtt
]


def main() -> None:
    rows = []
    for name, source_lang, target_lang in CASES:
        f = Path(name)
        rows.append(
            {
                "file": name,
                "source_lang": source_lang,
                "target_lang": target_lang,
                "is_subtitle": is_subtitle_file(f),
                "detected_source": detect_source_language(f, source_lang),
                "output_path": _output_path(f, target_lang),
            }
        )
    write_json("cli/translate_filename_cases.json", rows)


if __name__ == "__main__":
    main()
