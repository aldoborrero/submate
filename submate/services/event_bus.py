"""Event bus for real-time updates and inter-component communication."""

import logging
import uuid
from collections import defaultdict
from collections.abc import Callable
from dataclasses import dataclass
from datetime import datetime
from typing import Any

logger = logging.getLogger(__name__)

# Type alias for event handlers
EventHandler = Callable[[dict[str, Any]], None]


@dataclass
class Event:
    """Represents an event in the event bus.

    Attributes:
        type: The event type identifier.
        data: The event payload data.
        timestamp: When the event was created.
    """

    type: str
    data: dict[str, Any]
    timestamp: datetime


class EventBus:
    """Event bus for publishing and subscribing to events.

    This class provides a simple pub/sub mechanism for decoupled
    communication between components. Handlers are called synchronously
    when events are published.

    Example:
        bus = EventBus()

        def on_job_complete(data: dict) -> None:
            print(f"Job {data['job_id']} completed!")

        sub_id = bus.subscribe("job_complete", on_job_complete)
        bus.publish("job_complete", {"job_id": "123"})
        bus.unsubscribe("job_complete", sub_id)
    """

    def __init__(self) -> None:
        """Initialize an empty event bus."""
        self._subscribers: dict[str, dict[str, EventHandler]] = defaultdict(dict)

    def subscribe(self, event_type: str, handler: EventHandler) -> str:
        """Subscribe a handler to an event type.

        Args:
            event_type: The type of event to listen for.
            handler: The callback function to invoke when the event occurs.

        Returns:
            A unique subscription ID that can be used to unsubscribe.
        """
        sub_id = str(uuid.uuid4())
        self._subscribers[event_type][sub_id] = handler
        logger.debug(f"Subscribed handler {sub_id} to event type '{event_type}'")
        return sub_id

    def unsubscribe(self, event_type: str, sub_id: str) -> bool:
        """Unsubscribe a handler by its subscription ID.

        Args:
            event_type: The event type the handler was subscribed to.
            sub_id: The subscription ID returned from subscribe().

        Returns:
            True if the handler was found and removed, False otherwise.
        """
        if event_type not in self._subscribers:
            return False

        if sub_id not in self._subscribers[event_type]:
            return False

        del self._subscribers[event_type][sub_id]
        logger.debug(f"Unsubscribed handler {sub_id} from event type '{event_type}'")
        return True

    def publish(self, event_type: str, data: dict[str, Any]) -> None:
        """Publish an event to all subscribers.

        Args:
            event_type: The type of event being published.
            data: The event payload data.

        Note:
            Handler exceptions are logged but not re-raised to prevent
            one handler from breaking others.
        """
        handlers = self._subscribers.get(event_type, {})

        logger.debug(f"Publishing event '{event_type}' to {len(handlers)} subscriber(s)")

        for sub_id, handler in handlers.items():
            try:
                handler(data)
            except Exception:
                logger.exception(f"Error in event handler {sub_id} for event type '{event_type}'")


# Global singleton instance
_event_bus_instance: EventBus | None = None


def get_event_bus() -> EventBus:
    """Get or create the global EventBus singleton instance.

    Returns:
        The global EventBus instance.
    """
    global _event_bus_instance
    if _event_bus_instance is None:
        _event_bus_instance = EventBus()
    return _event_bus_instance
