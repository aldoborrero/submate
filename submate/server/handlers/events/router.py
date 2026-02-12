"""SSE Events API router for real-time updates."""

import asyncio
import json
import logging
from collections.abc import AsyncGenerator
from typing import Any

from fastapi import APIRouter
from fastapi.responses import StreamingResponse

from submate.services.event_bus import EventHandler, get_event_bus

logger = logging.getLogger(__name__)

# Event types that are streamed via SSE
SSE_EVENT_TYPES = [
    "job.created",
    "job.started",
    "job.completed",
    "job.failed",
    "sync.completed",
]


async def event_stream() -> AsyncGenerator[str]:
    """Generate SSE events from the EventBus.

    This async generator subscribes to relevant events from the EventBus
    and yields them in SSE format. It uses an asyncio.Queue to bridge
    the sync EventBus handlers to the async generator.

    Yields:
        SSE-formatted event strings.
    """
    event_bus = get_event_bus()
    queue: asyncio.Queue[dict[str, Any]] = asyncio.Queue()
    subscription_ids: dict[str, str] = {}

    # Capture the event loop reference for thread-safe queue operations
    loop = asyncio.get_running_loop()

    def create_handler(event_type: str) -> EventHandler:
        """Create a handler that puts events into the async queue.

        Args:
            event_type: The type of event to handle.

        Returns:
            A handler function that queues events.
        """

        def handler(data: dict[str, Any]) -> None:
            # Use call_soon_threadsafe to schedule queue put on the event loop
            # This ensures thread-safe operation when called from sync context
            try:
                loop.call_soon_threadsafe(queue.put_nowait, {"event_type": event_type, "data": data})
            except RuntimeError:
                # Loop might be closed
                logger.warning(f"Event loop closed, dropping event: {event_type}")
            except Exception:
                logger.exception("Failed to queue event")

        return handler

    try:
        # Subscribe to all relevant event types
        for event_type in SSE_EVENT_TYPES:
            handler = create_handler(event_type)
            sub_id = event_bus.subscribe(event_type, handler)
            subscription_ids[event_type] = sub_id
            logger.debug(f"SSE subscribed to {event_type} with id {sub_id}")

        # Stream events as they arrive
        while True:
            try:
                # Wait for an event with timeout to allow checking for disconnection
                event = await asyncio.wait_for(queue.get(), timeout=30.0)
                # Format as SSE
                event_line = f"event: {event['event_type']}\n"
                data_line = f"data: {json.dumps(event)}\n\n"
                yield event_line + data_line
            except TimeoutError:
                # Send a keep-alive comment to prevent connection timeout
                yield ": keep-alive\n\n"

    except asyncio.CancelledError:
        logger.debug("SSE stream cancelled")
        raise
    finally:
        # Clean up subscriptions
        for event_type, sub_id in subscription_ids.items():
            event_bus.unsubscribe(event_type, sub_id)
            logger.debug(f"SSE unsubscribed from {event_type}")


def create_events_router() -> APIRouter:
    """Create events API router.

    Returns:
        APIRouter with SSE events endpoint.
    """
    router = APIRouter(prefix="/api", tags=["events"])

    @router.get("/events")
    async def events() -> StreamingResponse:
        """Server-Sent Events endpoint for real-time updates.

        This endpoint streams events to the client in SSE format.
        Events include job status updates and sync completion notifications.

        Returns:
            StreamingResponse with text/event-stream content type.
        """
        return StreamingResponse(
            event_stream(),
            media_type="text/event-stream",
            headers={
                "Cache-Control": "no-cache",
                "Connection": "keep-alive",
                "X-Accel-Buffering": "no",  # Disable nginx buffering
            },
        )

    return router
