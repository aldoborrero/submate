"""Services package for Submate.

Note: JellyfinSyncService is imported lazily via __getattr__ because it
depends on SQLAlchemy which may not be available in all environments.
"""

from __future__ import annotations

from typing import TYPE_CHECKING

from submate.services.event_bus import Event, EventBus, EventHandler, get_event_bus
from submate.services.scanner import (
    LANGUAGE_CODES,
    SUBTITLE_EXTENSIONS,
    SubtitleScanner,
)

if TYPE_CHECKING:
    from submate.services.sync import JellyfinSyncService

__all__ = [
    "Event",
    "EventBus",
    "EventHandler",
    "get_event_bus",
    "JellyfinSyncService",
    "LANGUAGE_CODES",
    "SUBTITLE_EXTENSIONS",
    "SubtitleScanner",
]


def __getattr__(name: str) -> type:
    """Lazy import for JellyfinSyncService to avoid SQLAlchemy dependency at import time."""
    if name == "JellyfinSyncService":
        from submate.services.sync import JellyfinSyncService

        return JellyfinSyncService
    raise AttributeError(f"module {__name__!r} has no attribute {name!r}")
