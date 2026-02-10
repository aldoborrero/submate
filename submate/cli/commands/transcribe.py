"""Transcribe command and helpers."""

from pathlib import Path
from typing import TYPE_CHECKING

import click

from submate.config import get_config

if TYPE_CHECKING:
    from submate.config import Config
from submate.media_servers.jellyfin import JellyfinClient
from submate.paths import AUDIO_EXTENSIONS, VIDEO_EXTENSIONS, is_audio_file, is_video_file
from submate.queue import get_task_queue
from submate.queue.models import TranscriptionSkippedError
from submate.queue.tasks import TranscriptionTask

from ..utils import console, setup_logging


def format_supported_extensions(extensions: set[str]) -> str:
    """Format set of extensions for display (remove dots, sort alphabetically)."""
    return ", ".join(sorted(ext.lstrip(".") for ext in extensions))


@click.command()
@click.argument("path", type=click.Path())
@click.option(
    "--audio-language",
    "-a",
    help="Select audio track by language code (e.g., 'ja' for Japanese dub)",
)
@click.option(
    "--translate-to",
    "-t",
    help="Translate subtitles to target language (e.g., 'es', 'fr', 'de'). Uses LLM backend.",
)
@click.option("--force", "-f", is_flag=True, help="Overwrite existing subtitle files")
@click.option(
    "--log-level",
    type=click.Choice(["DEBUG", "INFO", "WARNING", "ERROR"], case_sensitive=False),
    default="INFO",
    help="Set logging level (DEBUG, INFO, WARNING, ERROR)",
)
@click.option(
    "--log-file",
    type=click.Path(writable=True),
    help="Write logs to specified file (in addition to console)",
)
@click.option("--recursive", "-r", is_flag=True, help="Process subdirectories recursively")
@click.option("--fail-fast", is_flag=True, help="Stop immediately on first error")
@click.option("--sync", is_flag=True, help="Process files immediately (synchronous)")
@click.option(
    "--refresh-jellyfin", is_flag=True, help="Refresh Jellyfin libraries after processing (requires sync mode)"
)
@click.pass_context
def transcribe(
    ctx: click.Context,
    path: str,
    audio_language: str | None,
    translate_to: str | None,
    force: bool,
    log_level: str,
    log_file: str | None,
    recursive: bool,
    fail_fast: bool,
    sync: bool,
    refresh_jellyfin: bool,
) -> None:
    """Transcribe video or audio files to generate subtitles.

    By default, files are queued for processing by a worker. Use --sync
    to process immediately without requiring a worker.

    PATH can be a single file or a directory.

    Examples:
        submate transcribe movie.mp4 --sync
        submate transcribe movie.mkv --audio-language ja --sync
        submate transcribe movie.mkv --audio-language ja --translate-to es --sync
        submate transcribe /media/movies --recursive
    """
    # Setup logging
    setup_logging(False, log_level, log_file)

    # Get config from context
    config_file = ctx.obj.get("config_file")
    config = get_config(config_file)
    path_obj = Path(path)

    # Collect and validate files to process
    files_to_process = []
    skipped_files = []

    if path_obj.is_file():
        if is_video_file(path_obj) or is_audio_file(path_obj):
            files_to_process.append(path_obj)
        else:
            ext = path_obj.suffix.lower()
            console.print(f"[red]Error:[/red] Unsupported file type: {ext}")
            console.print(f"[dim]Supported video: {format_supported_extensions(VIDEO_EXTENSIONS)}[/dim]")
            console.print(f"[dim]Supported audio: {format_supported_extensions(AUDIO_EXTENSIONS)}[/dim]")
            raise click.Abort()
    elif path_obj.is_dir():
        console.print(f"[cyan]Scanning directory: {path}[/cyan]")
        pattern = "**/*" if recursive else "*"
        for file in path_obj.glob(pattern):
            if file.is_file():
                if is_video_file(file) or is_audio_file(file):
                    files_to_process.append(file)
                elif not file.name.startswith(".") and file.suffix.lower() not in {
                    ".txt",
                    ".jpg",
                    ".png",
                    ".nfo",
                    ".srt",
                    ".vtt",
                }:
                    skipped_files.append(file)

        if recursive and len(files_to_process) > 100:
            console.print(f"[yellow]Warning:[/yellow] Found {len(files_to_process)} files. This may take a while.")
            if not click.confirm("Continue?", default=True):
                raise click.Abort()
    else:
        console.print(f"[red]Error:[/red] Path does not exist: {path}")
        raise click.Abort()

    if not files_to_process:
        console.print("[yellow]No supported media files found[/yellow]")
        if skipped_files:
            console.print(f"[dim]Scanned {len(skipped_files)} other files[/dim]")
        return

    console.print(f"[green]Found {len(files_to_process)} media file(s) to process[/green]")
    if skipped_files:
        console.print(f"[dim]Skipped {len(skipped_files)} non-media files[/dim]")

    # Use TaskQueue with immediate parameter for sync mode
    if sync:
        console.print("[cyan]Processing files synchronously...[/cyan]\n")
    else:
        console.print("\n[cyan]Queueing files for worker processing...[/cyan]")
        console.print("[yellow]Note:[/yellow] Start worker with: submate worker\n")

    # Enqueue all files (will process immediately if sync mode)
    _enqueue_files(
        files_to_process=files_to_process,
        audio_language=audio_language,
        translate_to=translate_to,
        force=force,
        immediate=sync,
    )

    # Refresh Jellyfin if requested (only makes sense in sync mode)
    if sync and refresh_jellyfin:
        _refresh_jellyfin_libraries(config)


def _enqueue_files(
    files_to_process: list[Path],
    audio_language: str | None,
    translate_to: str | None,
    force: bool,
    immediate: bool = False,
) -> None:
    """Enqueue files for processing (immediate if sync mode, queued otherwise)."""
    queued = 0
    skipped = 0
    failed = 0

    task_queue = get_task_queue()

    with console.status("[cyan]Processing files...[/cyan]") as status:
        for i, file in enumerate(files_to_process, 1):
            try:
                status.update(f"[cyan]Processing {i}/{len(files_to_process)}: {file.name}[/cyan]")

                result = task_queue.enqueue(
                    TranscriptionTask,
                    file_path=str(file),
                    audio_language=audio_language,
                    translate_to=translate_to,
                    force=force,
                    immediate=immediate,
                )

                # In immediate mode, check the TaskResult
                if immediate:
                    if result and hasattr(result, "success") and not result.success:
                        failed += 1
                        error_msg = getattr(result, "error", "Unknown error")
                        console.print(f"  [red]✗[/red] Failed: {file.name} - {error_msg}")
                        continue
                    console.print(f"  [green]✓[/green] Processed: {file.name}")
                else:
                    console.print(f"  [green]✓[/green] Queued: {file.name}")

                queued += 1

            except TranscriptionSkippedError as e:
                skipped += 1
                console.print(f"  [yellow]-[/yellow] Skipped: {file.name} ({e.reason.value})")

            except Exception as e:
                failed += 1
                console.print(f"  [red]✗[/red] Failed: {file.name} - {str(e)[:100]}")

    # Summary
    console.print("\n[bold]Results:[/bold]")
    console.print(f"  [green]Successful:[/green] {queued} files")
    if skipped > 0:
        console.print(f"  [yellow]Skipped:[/yellow] {skipped} files")
    if failed > 0:
        console.print(f"  [red]Failed:[/red] {failed} files")

    if not immediate:
        queue_size = task_queue.size
        console.print(f"  [blue]Queue status:[/blue] {queue_size} pending tasks")

    if failed > 0:
        console.print(f"\n[yellow]Note:[/yellow] {failed} files failed to process. Check logs for details.")
        if not force and not immediate:
            console.print(
                "[yellow]Use --force to overwrite existing subtitles, or --sync for immediate processing.[/yellow]"
            )

    if skipped > 0 and not force:
        console.print("\n[yellow]Tip:[/yellow] Use --force to process skipped files anyway.")


def _refresh_jellyfin_libraries(config: "Config") -> None:
    """Refresh Jellyfin libraries after transcription."""
    try:
        console.print("\n[cyan]Refreshing Jellyfin libraries...[/cyan]")
        jellyfin = JellyfinClient(config)
        if jellyfin.is_configured():
            jellyfin.connect()
            refreshed = jellyfin.refresh_all_libraries()
            console.print(f"  [green]✓[/green] Refreshed {len(refreshed)} Jellyfin libraries")
        else:
            console.print(
                "  [yellow]Warning:[/yellow] Jellyfin not configured (set JELLYFIN_SERVER_URL and JELLYFIN_API_KEY)"
            )
    except Exception as e:
        console.print(f"  [red]Error refreshing Jellyfin:[/red] {str(e)}")
