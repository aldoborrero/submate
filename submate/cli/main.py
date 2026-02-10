"""Main CLI group definition."""

import click

from submate import __version__

from .commands import config_group, server, transcribe, translate, worker


@click.group()
@click.version_option(version=__version__, prog_name="submate")
@click.option(
    "--config-file",
    "-c",
    type=click.Path(exists=True),
    help="Path to configuration file (.env or .toml)",
)
@click.pass_context
def cli(ctx: click.Context, config_file: str | None) -> None:
    """Submate - AI-powered subtitle generation using Whisper.

    Generate subtitles for videos using stable-ts.
    Configuration is read from environment variables or config file.

    Examples:
        submate transcribe movie.mp4
        submate --config-file production.env transcribe movie.mp4
        submate config show
    """
    ctx.ensure_object(dict)
    ctx.obj["config_file"] = config_file


# Register command groups
cli.add_command(config_group, name="config")
cli.add_command(transcribe)
cli.add_command(translate)
cli.add_command(worker)
cli.add_command(server)
