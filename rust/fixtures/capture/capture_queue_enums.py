"""Capture queue/models enum .value strings -> fixtures/queue/enum_values.json.

Falsifier target: submate-queue parity::queue_enum_values.

Kept separate from capture_enums.py (which owns types/enum_values.json) because
the submate-types parity guard `no_uncovered_enums_in_golden` asserts its golden
contains *exactly* the six types.py enums. Queue enums live in their own golden
so neither crate's coverage guard fights the other.
"""

from __future__ import annotations

from _common import write_json

from submate.queue.models import OutputFormat, SkipReason

ENUMS = [
    OutputFormat,
    SkipReason,
]


def main() -> None:
    out: dict[str, dict[str, str]] = {}
    for enum in ENUMS:
        out[enum.__name__] = {member.name: str(member.value) for member in enum}
    write_json("queue/enum_values.json", out)


if __name__ == "__main__":
    main()
