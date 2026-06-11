"""Capture LanguageCode conversions -> fixtures/lang/lang_conversions.json.

Falsifier target: submate-lang parity::lang_conversions. Covers all variants,
both directions (enum -> iso/name, and iso/name -> enum via from_string).
"""

from __future__ import annotations

from _common import write_json

from submate.language import LanguageCode


def main() -> None:
    rows = []
    for lang in LanguageCode:
        rows.append(
            {
                "name": lang.name,
                "iso_639_1": lang.to_iso_639_1(),
                "iso_639_2_t": lang.to_iso_639_2_t(),
                "iso_639_2_b": lang.to_iso_639_2_b(),
                "name_en": lang.to_name(in_english=True),
                "name_native": lang.to_name(in_english=False),
                # round-trips: each accessor string must resolve back to this variant
                "from_iso_639_1": LanguageCode.from_string(lang.to_iso_639_1()).name if lang.to_iso_639_1() else None,
                "from_name_en": LanguageCode.from_string(lang.to_name(in_english=True)).name
                if lang.to_name(in_english=True)
                else None,
            }
        )
    write_json("lang/lang_conversions.json", rows)


if __name__ == "__main__":
    main()
