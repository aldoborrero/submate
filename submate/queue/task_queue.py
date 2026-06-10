"""TaskQueue using Huey as task registry with service injection."""

import logging
import threading
from typing import Any

from huey import SqliteHuey

from submate.config import Config
from submate.queue.tasks.base import BaseTask

logger = logging.getLogger(__name__)

# Global singleton Huey instance
_global_huey: SqliteHuey | None = None
_huey_lock = threading.Lock()


def get_huey() -> SqliteHuey:
    """Get the global singleton Huey instance.

    This returns the same Huey instance across all calls, ensuring
    tasks registered in one module are visible to all others.
    """
    global _global_huey
    if _global_huey is None:
        with _huey_lock:
            if _global_huey is None:
                from submate.config import get_config

                config = get_config()
                _global_huey = SqliteHuey(
                    name="submate",
                    filename=config.queue.db_path,
                    results=True,
                    utc=True,
                )
    return _global_huey


class TaskQueue:
    """Task queue using Huey as the task registry.

    Provides service injection for transcription and Bazarr operations
    while maintaining clean separation from queue infrastructure.
    """

    def __init__(self, config: Config | None = None):
        if config is None:
            from submate.config import get_config

            config = get_config()
        self.config = config

        # Initialize services
        from .services import BazarrService, TranscriptionService

        self.transcription_service = TranscriptionService(config)
        self.bazarr_service = BazarrService(config)

    @property
    def huey(self) -> SqliteHuey:
        """Get the global Huey queue instance."""
        return get_huey()

    def enqueue(
        self, task_class: type[BaseTask], blocking: bool = False, immediate: bool = False, **kwargs: Any
    ) -> Any:
        """Enqueue a task for execution.

        Args:
            task_class: The task class to execute
            blocking: Wait for task completion
            immediate: Execute synchronously (skip queue)
            **kwargs: Task-specific arguments

        Returns:
            Task result (if blocking) or Huey result object
        """
        # Create task instance with services injected
        task = task_class(
            config=self.config,
            transcription_service=self.transcription_service,
            bazarr_service=self.bazarr_service,
        )

        # Validate input
        task.validate_input(**kwargs)

        # Log task ID for debugging (Huey doesn't support deduplication)
        task_id = task.get_task_id(**kwargs)
        logger.debug("Task ID: %s", task_id)

        # Handle immediate mode (synchronous execution)
        if immediate:
            # Temporarily set immediate mode for sync processing
            original_immediate = self.huey.immediate
            self.huey.immediate = True
            try:
                # Execute task directly without queuing
                return task.execute(**kwargs)
            finally:
                # Always restore original immediate setting
                self.huey.immediate = original_immediate
        else:
            # Dispatch to a statically-registered Huey task. The worker process
            # only knows tasks registered at import time, so registering a new
            # per-call task here would be invisible to it (and a lambda body is
            # not serializable). Pass only plain serializable kwargs; the worker
            # reconstructs its own services from config.
            huey_task = self._registered_task_for(task)
            result = huey_task(**kwargs)

            if blocking:
                return result()  # Wait for completion
            return result

    def _registered_task_for(self, task: BaseTask) -> Any:
        """Return the statically-registered Huey task used to run a queued task.

        Queued (non-immediate) execution must go through a task registered at
        import time so a separate worker process can find it in its registry.
        """
        from submate.queue import registered_tasks

        registered = {
            "transcription": registered_tasks.transcribe_file_task,
        }
        huey_task = registered.get(task.task_name)
        if huey_task is None:
            raise ValueError(
                f"No statically-registered Huey task for '{task.task_name}'. "
                "Queued execution requires a task registered in registered_tasks.py; "
                "use immediate=True or add a registered task."
            )
        return huey_task

    @property
    def size(self) -> int:
        """Number of pending tasks in the queue."""
        try:
            queue_size = self.huey.storage.queue_size()
            return int(queue_size) if queue_size is not None else 0
        except Exception:
            logger.error("Failed to get queue size", exc_info=True)
            return 0

    @property
    def stats(self) -> dict[str, int]:
        """Queue statistics including pending and scheduled tasks."""
        try:
            storage = self.huey.storage
            pending = storage.queue_size()
            scheduled = storage.schedule_size()

            return {
                "pending": pending if isinstance(pending, int) else 0,
                "scheduled": scheduled if isinstance(scheduled, int) else 0,
            }
        except Exception as e:
            logger.error("Failed to get queue stats: %s", e, exc_info=True)
            return {"pending": 0, "scheduled": 0}

    def cancel(self, task_id: str) -> bool:
        """Cancel a specific task by ID."""
        try:
            result = self.huey.revoke_by_id(task_id)
            return bool(result) if result is not None else False
        except Exception as e:
            logger.error("Failed to cancel task %s: %s", task_id, e, exc_info=True)
            return False

    def cancel_all(self) -> int:
        """Cancel all pending tasks."""
        cancelled = 0
        try:
            pending_tasks = self.huey.pending()
            for task in pending_tasks:
                if self.huey.revoke_by_id(task.id):
                    cancelled += 1
            return cancelled
        except Exception as e:
            logger.error("Failed to cancel all tasks: %s", e, exc_info=True)
            return 0

    def clear(self) -> int:
        """Clear all pending tasks from the queue.

        Returns:
            Number of tasks cleared
        """
        try:
            storage = self.huey.storage
            count = storage.queue_size()
            if isinstance(count, int) and count > 0:
                storage.flush_queue()
                logger.info("Cleared %s tasks from queue", count)
                return count
            return 0
        except Exception as e:
            logger.error("Failed to clear queue: %s", e, exc_info=True)
            return 0
