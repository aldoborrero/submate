"""Capture the golden for the Bazarr detect-language naming port.

Drives the *real* `BazarrService.LANGUAGE_NAMES` table and the two-step
naming logic from `submate/queue/services/bazarr.py` (the post-transcription
code path, with the whisper call dropped — we feed it detected-code strings
directly):

    1. language_code = whisper_lang or "und"   (Python truthiness: None/"" -> "und")
    2. language_name = LANGUAGE_NAMES.get(language_code, "Unknown")
    3. -> {"detected_language": language_name, "language_code": language_code}

The table is a deliberately NARROW hardcoded map; any out-of-set code (even a
valid ISO-639-1 code) names "Unknown". This script sources the names from the
live class attribute so the golden cannot drift from the Python spec.

This script is NOT part of the grind. A human runs it once (and again if the
Python table changes), then commits:

    rust/fixtures/queue/bazarr_language_names.json
"""

from __future__ import annotations

from submate.queue.services.bazarr import BazarrService

from _common import write_json

# Input cases the falsifier pins: the in-set codes, several out-of-set but
# valid ISO codes, the absent-detection cases, and a bogus code. Each entry is
# the *raw whisper-detected language* (None / "" model the no-detection paths).
IN_SET = [
    "en", "es", "fr", "de", "it", "pt", "ru", "ja", "zh", "ko",
    "ar", "hi", "nl", "pl", "tr", "vi", "th", "sv", "da", "fi",
    "no", "cs", "el", "he", "hu", "id", "ms", "ro", "sk", "uk",
]
OUT_OF_SET = ["ca", "fa", "be", "xx"]
ABSENT = [None, ""]


def name_pair(whisper_lang: str | None) -> dict[str, str]:
    """Byte-for-byte mirror of BazarrService.detect_language's naming logic."""
    language_code = whisper_lang or "und"
    language_name = BazarrService.LANGUAGE_NAMES.get(language_code, "Unknown")
    return {"detected_language": language_name, "language_code": language_code}


def main() -> None:
    cases = []
    for code in IN_SET + OUT_OF_SET:
        cases.append({"input": code, "expected": name_pair(code)})
    for absent in ABSENT:
        cases.append({"input": absent, "expected": name_pair(absent)})

    write_json(
        "queue/bazarr_language_names.json",
        {
            "language_names": dict(BazarrService.LANGUAGE_NAMES),
            "cases": cases,
        },
    )


if __name__ == "__main__":
    main()
