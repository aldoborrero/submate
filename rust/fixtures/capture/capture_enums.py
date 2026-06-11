"""Capture enum .value strings -> fixtures/types/enum_values.json.

Falsifier target: submate-types parity::enum_values.
"""

from __future__ import annotations

from _common import write_json

from submate import types as t

ENUMS = [
    t.WhisperModel,
    t.WhisperImplementation,
    t.Device,
    t.TranscriptionTask,
    t.LanguageNamingType,
    t.TranslationBackend,
]


def main() -> None:
    out: dict[str, dict[str, str]] = {}
    for enum in ENUMS:
        out[enum.__name__] = {member.name: str(member.value) for member in enum}
    write_json("types/enum_values.json", out)


if __name__ == "__main__":
    main()
