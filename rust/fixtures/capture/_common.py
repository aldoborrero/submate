"""Shared helpers for the fixture-capture scripts.

These scripts run against the *Python* submate to emit the golden fixtures the
Rust port is tested against. They are NOT part of the grind — a human runs them
once (and again whenever the Python spec changes), then commits rust/fixtures/.
"""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

# capture/ lives at rust/fixtures/capture, so the fixtures root is its parent.
FIXTURES = Path(__file__).resolve().parent.parent


def write_json(rel: str, data: Any) -> None:
    """Write golden JSON deterministically (sorted keys, stable formatting).

    Formatting is irrelevant to the Rust side (it compares parsed serde_json
    Values), but stable output keeps git diffs meaningful and re-runs idempotent.
    """
    path = FIXTURES / rel
    path.parent.mkdir(parents=True, exist_ok=True)
    text = json.dumps(data, indent=2, sort_keys=True, ensure_ascii=False)
    path.write_text(text + "\n", encoding="utf-8")
    print(f"wrote {rel}")


def write_text(rel: str, text: str) -> None:
    """Write a golden text file (e.g. an .srt/.vtt/.env) verbatim."""
    path = FIXTURES / rel
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")
    print(f"wrote {rel}")
