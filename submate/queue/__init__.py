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

import threading
from typing import Any

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

# Global singleton instance, guarded by a lock (double-checked) so concurrent
# first-callers don't each build a TaskQueue and its services.
_task_queue: TaskQueue | None = None
_task_queue_lock = threading.Lock()


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
        with _task_queue_lock:
            if _task_queue is None:
                from submate.config import get_config

                _task_queue = TaskQueue(get_config())
    return _task_queue


# For backwards compatibility, provide huey as a property-like access
# Import this module and access submate.queue.huey to get the instance
def __getattr__(name: str) -> Any:
    """Lazy attribute access for module-level huey instance."""
    if name == "huey":
        # get_huey() is itself a lock-guarded process singleton, so no extra
        # caching (or extra race) is needed here.
        return get_huey()
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
