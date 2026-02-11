"""Worker command for background processing."""

from pathlib import Path

import click
from huey.consumer import Consumer

import submate.queue.registered_tasks  # noqa: F401 (side effect: registers tasks)
from submate.queue.task_queue import get_huey

from ..utils import console, setup_logging


@click.command()
@click.option("--workers", "-w", type=int, default=2, help="Number of worker threads")
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
@click.option("--daemonize", "-d", is_flag=True, help="Run as background daemon")
def worker(workers: int, log_level: str, log_file: click.Path | None, daemonize: bool) -> None:
    """Start the Huey task worker for processing transcriptions.

    The worker processes tasks from the Huey queue. Run this alongside
    the webhook server or when using batch processing.

    When daemonized, logs are written to ~/.submate/worker.log and a PID
    file is created at ~/.submate/worker.pid.

    Examples:
        submate worker
        submate worker --workers 4 --log-level DEBUG
        submate worker --daemonize
    """
    if daemonize:
        from daemon import DaemonContext
        from daemon.pidfile import TimeoutPIDLockFile

        pid_dir = Path.home() / ".submate"
        pid_dir.mkdir(exist_ok=True, parents=True)
        pidfile = pid_dir / "worker.pid"
        daemon_log_file = pid_dir / "worker.log"

        console.print("[cyan]Starting Huey worker as daemon...[/cyan]")
        console.print(f"  PID file: {pidfile}")
        console.print(f"  Log file: {daemon_log_file}")
        console.print(f"  Workers: {workers}")
        console.print(f"  Log level: {log_level}\n")

        # Open file handles that will be properly managed by DaemonContext
        stdout_file = open(str(daemon_log_file), "a")
        stderr_file = open(str(daemon_log_file), "a")

        with DaemonContext(
            pidfile=TimeoutPIDLockFile(str(pidfile)),
            working_directory=str(Path.cwd()),
            stdout=stdout_file,
            stderr=stderr_file,
            files_preserve=[stdout_file, stderr_file],
        ):
            _run_worker(workers, log_level, str(daemon_log_file))
    else:
        _run_worker(workers, log_level, str(log_file) if log_file else None)


def _run_worker(workers: int, log_level: str, log_file: str | None) -> None:
    """Run the Huey worker with the specified configuration."""
    # Setup logging based on log level and file
    setup_logging(False, log_level, log_file)

    huey = get_huey()

    if not Path.home().joinpath(".submate").exists():
        console.print("[cyan]Starting Huey worker...[/cyan]")
        console.print(f"  Workers: {workers}")
        console.print(f"  Log level: {log_level}\n")

    # Create and run Huey consumer directly
    consumer = Consumer(
        huey,
        workers=workers,
        periodic=True,
        initial_delay=0.1,
        check_worker_health=True,
        health_check_interval=10,
    )

    try:
        consumer.run()
    except KeyboardInterrupt:
        console.print("\n[yellow]Worker stopped[/yellow]")
