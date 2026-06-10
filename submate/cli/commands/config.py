"""Config management commands."""

from typing import Any

import click
from rich.table import Table

from submate.config import get_config

from ..utils import console


def _format_value(value: Any) -> str:
    """Render a leaf config value for display."""
    if isinstance(value, list):
        return ", ".join(str(item) for item in value) if value else "(none)"
    if isinstance(value, bool):
        return "Yes" if value else "No"
    if value == "" or value is None:
        return "(not set)"
    return str(value)


def _flatten_settings(value: Any, prefix: str = "") -> list[tuple[str, str]]:
    """Flatten nested settings dicts into (dotted name, display value) rows."""
    if isinstance(value, dict):
        rows: list[tuple[str, str]] = []
        for key, nested in value.items():
            name = f"{prefix}.{key}" if prefix else key
            rows.extend(_flatten_settings(nested, name))
        return rows
    return [(prefix, _format_value(value))]


@click.group()
def config_group() -> None:
    """Manage configuration settings."""
    pass


@config_group.command()
@click.pass_context
def show(ctx: click.Context) -> None:
    """Show current configuration from environment variables."""
    config_file = ctx.obj.get("config_file")
    cfg = get_config(config_file)

    table = Table(title="Submate Configuration")
    table.add_column("Setting", style="cyan")
    table.add_column("Value", style="green")

    # Flatten nested settings groups (whisper, server, ...) into dotted rows.
    # model_dump(mode="json") serializes enums to their values so they render
    # cleanly instead of as Python reprs.
    for name, display_value in _flatten_settings(cfg.model_dump(mode="json")):
        display_name = ".".join(part.replace("_", " ").title() for part in name.split("."))
        table.add_row(display_name, display_value)

    console.print(table)
