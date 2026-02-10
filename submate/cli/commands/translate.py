"""Translate subtitle files using LLM backends."""

import logging
from pathlib import Path

import click

from submate.cli.utils import console, setup_logging
from submate.config import get_config
from submate.translation import TranslationService

logger = logging.getLogger(__name__)

SUBTITLE_EXTENSIONS = {".srt", ".vtt", ".ass", ".ssa"}


def is_subtitle_file(path: Path) -> bool:
    """Check if path is a subtitle file."""
    return path.suffix.lower() in SUBTITLE_EXTENSIONS


def find_subtitle_files(path: Path, recursive: bool = False) -> list[Path]:
    """Find subtitle files in path."""
    if path.is_file():
        return [path] if is_subtitle_file(path) else []

    pattern = "**/*" if recursive else "*"
    return [f for f in path.glob(pattern) if f.is_file() and is_subtitle_file(f)]


@click.command()
@click.argument("path", type=click.Path(exists=True))
@click.option("--source-lang", "-s", default="auto", help="Source language (default: auto-detect from filename)")
@click.option("--target-lang", "-t", required=True, help="Target language code (e.g., 'es', 'fr', 'de')")
@click.option("--output", "-o", type=click.Path(), help="Output file path (default: input.{target}.srt)")
@click.option("--recursive", "-r", is_flag=True, help="Process directories recursively")
@click.option("--force", "-f", is_flag=True, help="Overwrite existing output files")
@click.option("--log-level", type=click.Choice(["DEBUG", "INFO", "WARNING", "ERROR"]), default="INFO")
@click.pass_context
def translate(
    ctx: click.Context,
    path: str,
    source_lang: str,
    target_lang: str,
    output: str | None,
    recursive: bool,
    force: bool,
    log_level: str,
) -> None:
    """Translate subtitle files to target language using LLM.

    PATH can be a subtitle file (.srt, .vtt, .ass, .ssa) or a directory.
    ASS/SSA files preserve all formatting tags during translation.

    Examples:
        submate translate movie.srt -t es
        submate translate movie.en.srt -t fr -o movie.fr.srt
        submate translate anime.ja.ass -t en
        submate translate ./subtitles/ -t de -r
    """
    setup_logging(False, log_level, None)

    config_file = ctx.obj.get("config_file")
    config = get_config(config_file)

    # Validate LLM backend configuration
    config.translation.validate_for_target(target_lang)

    input_path = Path(path)
    files = find_subtitle_files(input_path, recursive)

    if not files:
        console.print(f"[red]No subtitle files found in {path}[/red]")
        raise click.Abort()

    service = TranslationService(config)

    for file in files:
        # Determine output path
        if output and len(files) == 1:
            output_path = Path(output)
        else:
            # movie.srt -> movie.es.srt or movie.en.srt -> movie.es.srt
            stem = file.stem
            if "." in stem:
                # Remove existing language suffix (e.g., movie.en -> movie)
                base = stem.rsplit(".", 1)[0]
            else:
                base = stem
            output_path = file.parent / f"{base}.{target_lang}{file.suffix}"

        if output_path.exists() and not force:
            console.print(f"[yellow]Skipping {file} - output exists (use -f to overwrite)[/yellow]")
            continue

        console.print(f"[blue]Translating {file.name} -> {target_lang}[/blue]")

        try:
            content = file.read_text(encoding="utf-8")

            # Auto-detect source language from filename if not specified
            detected_source = source_lang
            if source_lang == "auto" and "." in file.stem:
                detected_source = file.stem.rsplit(".", 1)[-1]
                if len(detected_source) > 3:  # Not a language code
                    detected_source = "en"  # Default fallback
            elif source_lang == "auto":
                detected_source = "en"

            # Use appropriate translation method based on file type
            if file.suffix.lower() in {".ass", ".ssa"}:
                translated = service.translate_ass_content(
                    content,
                    source_lang=detected_source,
                    target_lang=target_lang,
                )
            else:
                translated = service.translate_srt_content(
                    content,
                    source_lang=detected_source,
                    target_lang=target_lang,
                )

            output_path.write_text(translated, encoding="utf-8")
            console.print(f"[green]Saved {output_path.name}[/green]")

        except Exception as e:
            logger.error(f"Failed to translate {file}: {e}", exc_info=True)
            console.print(f"[red]Failed: {e}[/red]")
