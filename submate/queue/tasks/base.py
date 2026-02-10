from abc import ABC, abstractmethod
from typing import Any, TypeVar

from submate.config import Config
from submate.queue.models import TaskResult

TResult = TypeVar("TResult")


class BaseTask[TResult](ABC):
    """Abstract base for all queue tasks."""

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
    def execute(self, **kwargs: Any) -> TaskResult[TResult]:
        """Execute the task. Return result, don't raise exceptions."""
        pass

    def validate_input(self, **kwargs: Any) -> None:
        """Validate input parameters before execution."""
        pass

    def get_task_id(self, **kwargs: Any) -> str:
        """Generate unique ID for deduplication."""
        import hashlib

        key = f"{self.task_name}:{sorted(kwargs.items())}"
        return hashlib.sha256(str(key).encode()).hexdigest()[:16]
