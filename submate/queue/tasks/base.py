import logging
from abc import ABC, abstractmethod
from typing import Any

from submate.config import Config
from submate.queue.models import TaskResult

logger = logging.getLogger(__name__)


class BaseTask[TResult](ABC):
    """Abstract base for all queue tasks.

    Subclasses implement :meth:`_run` with the real work. The base wraps it in a
    uniform try/except that returns a :class:`TaskResult`, logs failures, and
    lets declared exceptions propagate.
    """

    #: Exception types that should propagate out of execute() unwrapped.
    propagate_exceptions: tuple[type[BaseException], ...] = ()
    #: Data attached to the TaskResult when the task fails.
    failure_data: Any = None

    def __init__(self, config: Config, **dependencies: Any) -> None:
        self.config = config
        # Inject dependencies (services, etc.)
        for name, dep in dependencies.items():
            setattr(self, name, dep)

    @property
    @abstractmethod
    def task_name(self) -> str:
        """Unique task identifier for registry and logging."""
        pass

    @abstractmethod
    def _run(self, **kwargs: Any) -> TResult:
        """Perform the task and return its result data, or raise on failure."""
        pass

    def execute(self, **kwargs: Any) -> TaskResult[TResult]:
        """Run the task, wrapping the outcome in a TaskResult."""
        try:
            data = self._run(**kwargs)
            return TaskResult(success=True, data=data)
        except self.propagate_exceptions:
            raise
        except Exception as e:
            logger.error("Task '%s' failed", self.task_name, exc_info=True)
            return TaskResult(success=False, error=str(e), data=self.failure_data)

    def validate_input(self, **kwargs: Any) -> None:
        """Validate input parameters before execution."""
        pass

    def get_task_id(self, **kwargs: Any) -> str:
        """Generate a stable correlation id for this call.

        Used for logging and the queued task name only -- Huey does not support
        deduplication, so this is not used to suppress duplicate work.
        """
        import hashlib

        key = f"{self.task_name}:{sorted(kwargs.items())}"
        return hashlib.sha256(str(key).encode()).hexdigest()[:16]
