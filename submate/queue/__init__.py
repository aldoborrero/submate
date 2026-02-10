"""Queue system for async task processing.

This module provides a task queue system built on Huey for background
processing of transcription tasks.

Example:
    >>> from submate.queue import get_task_queue
    >>> from submate.queue.tasks import TranscriptionTask
    >>>
    >>> task_queue = get_task_queue()
    >>> result = task_queue.enqueue(
    ...     TranscriptionTask,
    ...     file_path="/path/to/video.mp4",
    ...     language="en"
    ... )
    >>>
    >>> # Access queue statistics
    >>> print(f"Pending: {task_queue.stats['pending']}")
    >>> print(f"Queue size: {task_queue.size}")
"""

from typing import TYPE_CHECKING, Any

if TYPE_CHECKING:
    from huey import SqliteHuey

# Core queue classes
# Data models
from .models import (
    OutputFormat,
    TaskResult,
    TranscriptionResult,
)
from .task_queue import TaskQueue, get_huey

# Task classes
from .tasks import (
    BazarrTranscriptionTask,
    LanguageDetectionTask,
    TranscriptionTask,
)

# Global singleton instance
_task_queue: TaskQueue | None = None


def get_task_queue() -> TaskQueue:
    """Get or create the global task queue instance.

    Returns:
        TaskQueue: The global task queue singleton

    Example:
        >>> task_queue = get_task_queue()
        >>> print(f"Pending: {task_queue.stats['pending']}")
        >>> print(f"Queue size: {task_queue.size}")
    """
    global _task_queue
    if _task_queue is None:
        from submate.config import get_config

        _task_queue = TaskQueue(get_config())
    return _task_queue


# Lazy Huey instance - only initialized when accessed
_huey_instance: "SqliteHuey | None" = None


def _get_lazy_huey() -> "SqliteHuey":
    """Get lazily-initialized Huey instance."""
    global _huey_instance
    if _huey_instance is None:
        _huey_instance = get_huey()
    return _huey_instance


# For backwards compatibility, provide huey as a property-like access
# Import this module and access submate.queue.huey to get the instance
def __getattr__(name: str) -> Any:
    """Lazy attribute access for module-level huey instance."""
    if name == "huey":
        return _get_lazy_huey()
    raise AttributeError(f"module {__name__!r} has no attribute {name!r}")


__all__ = [
    # Queue management
    "TaskQueue",
    "get_task_queue",
    "get_huey",
    "huey",
    # Task classes
    "BazarrTranscriptionTask",
    "LanguageDetectionTask",
    "TranscriptionTask",
    # Data models
    "OutputFormat",
    "TaskResult",
    "TranscriptionResult",
]
