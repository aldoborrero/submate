"""Config management commands."""

import click
from rich.table import Table

from submate.config import get_config

from ..utils import console


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

    # Use Pydantic's model_dump() to get all fields
    for field_name, value in cfg.model_dump().items():
        # Format field name nicely
        display_name = field_name.replace("_", " ").title()

        # Format value
        if isinstance(value, list):
            display_value = ", ".join(value) if value else "(none)"
        elif isinstance(value, bool):
            display_value = "Yes" if value else "No"
        elif value == "":
            display_value = "(not set)"
        else:
            display_value = str(value)

        table.add_row(display_name, display_value)

    console.print(table)
