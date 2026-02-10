"""Shared CLI utilities and helpers."""

import logging

from rich.console import Console

console = Console()


def setup_logging(verbose: bool = False, level: str = "INFO", log_file: str | None = None) -> None:
    """Setup logging configuration."""
    if verbose:
        # Backward compatibility (deprecated)
        log_level = logging.DEBUG
    else:
        log_level = getattr(logging, level.upper(), logging.INFO)

    # Configure logging format
    formatter = logging.Formatter("%(asctime)s - %(name)s - %(levelname)s - %(message)s")

    # Configure root logger
    root_logger = logging.getLogger()
    root_logger.setLevel(log_level)

    # Clear any existing handlers
    root_logger.handlers.clear()

    # Always add console handler
    console_handler = logging.StreamHandler()
    console_handler.setFormatter(formatter)
    root_logger.addHandler(console_handler)

    # Add file handler if log_file is specified
    if log_file:
        try:
            file_handler = logging.FileHandler(log_file)
            file_handler.setFormatter(formatter)
            root_logger.addHandler(file_handler)
        except OSError as e:
            # If file logging fails, log to console and continue
            console_handler.emit(
                logging.LogRecord(
                    "submate.cli.utils",
                    logging.WARNING,
                    __file__,
                    0,
                    f"Failed to setup log file {log_file}: {e}",
                    (),
                    None,
                )
            )
