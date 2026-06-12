"""Capture `submate config show` display rows -> fixtures/cli/config_show.*.rows.json.

Falsifier target: submate-cli config_show_rows. The Rust port reproduces the
exact ordered (setting, value) rows the Python `config show` command renders,
i.e. _flatten_settings(cfg.model_dump(mode="json")) with the per-segment
title-case display-name map applied. The Rich Table chrome is out of scope; the
ordered rows are the contract.

Two goldens:
- config_show.defaults.rows.json  -> a default Config() with no env set.
- config_show.overridden.rows.json -> a Config built from an env that exercises
  every _format_value branch (bool Yes/No, empty list "(none)", populated list,
  unset key "(not set)", plain string/number).
"""

from __future__ import annotations

import os

from _common import write_json

from submate.cli.commands.config import _flatten_settings, _format_value  # noqa: F401
from submate.config import Config


def _rows(cfg: Config) -> list[list[str]]:
    """Mirror the `config show` row build: flatten + title-case display name."""
    rows: list[list[str]] = []
    for name, display_value in _flatten_settings(cfg.model_dump(mode="json")):
        display_name = ".".join(part.replace("_", " ").title() for part in name.split("."))
        rows.append([display_name, display_value])
    return rows


# Env exercising every _format_value branch. Keys use the SUBMATE__ prefix with
# __ nesting (matching the Pydantic settings config). The capture only sets what
# it needs; the rest fall back to defaults.
OVERRIDE_ENV = {
    # plain string leaf -> str(value)
    "SUBMATE__WHISPER__MODEL": "large-v3",
    # numeric leaf -> str(value)
    "SUBMATE__SERVER__PORT": "9123",
    # a populated list (pipe-separated -> joined with ", ")
    "SUBMATE__SUBTITLE__SKIP_SUBTITLE_LANGUAGES": "eng|spa",
    # a bool flag -> "Yes"
    "SUBMATE__SUBTITLE__SKIP_UNKNOWN_LANGUAGE": "true",
}


def main() -> None:
    # Defaults: no SUBMATE__ env present. Snapshot/clear any inherited prefix env
    # so the "defaults" golden is reproducible regardless of the caller's shell.
    saved = {k: v for k, v in os.environ.items() if k.startswith("SUBMATE__")}
    for k in saved:
        del os.environ[k]
    try:
        write_json("cli/config_show.defaults.rows.json", _rows(Config()))

        for k, v in OVERRIDE_ENV.items():
            os.environ[k] = v
        write_json("cli/config_show.overridden.rows.json", _rows(Config()))
    finally:
        for k in list(os.environ):
            if k.startswith("SUBMATE__"):
                del os.environ[k]
        os.environ.update(saved)


if __name__ == "__main__":
    main()
