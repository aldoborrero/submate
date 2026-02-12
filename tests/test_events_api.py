"""Tests for SSE Events API endpoint."""

import asyncio
import json

import pytest

from submate.services.event_bus import EventBus, get_event_bus


@pytest.fixture
def event_bus() -> EventBus:
    """Get and reset the event bus for testing.

    Returns:
        The global EventBus instance.
    """
    bus = get_event_bus()
    # Clear any existing subscribers
    bus._subscribers.clear()
    return bus


def test_events_router_registered() -> None:
    """Test that /api/events route is registered in the app."""
    from submate.server import app

    # Check that the events endpoint is registered
    routes = [r.path for r in app.routes if hasattr(r, "path")]
    assert "/api/events" in routes


def test_events_router_creation() -> None:
    """Test that create_events_router returns a valid router."""
    from submate.server.handlers.events.router import create_events_router

    router = create_events_router()
    assert router is not None
    # Check router has the events route (with /api prefix from router)
    routes = [r.path for r in router.routes if hasattr(r, "path")]
    assert "/api/events" in routes


@pytest.mark.asyncio
async def test_event_stream_generator() -> None:
    """Test that event_stream generator works correctly."""
    from submate.server.handlers.events.router import event_stream

    # Reset event bus
    bus = get_event_bus()
    bus._subscribers.clear()

    # Start the event stream generator
    stream = event_stream()

    async def consume_one() -> str | None:
        async for line in stream:
            return line
        return None

    # Start consuming in background
    consumer_task = asyncio.create_task(consume_one())

    # Wait for subscriptions to be set up
    await asyncio.sleep(0.1)

    # Publish an event
    bus.publish("job.created", {"job_id": "test-123"})

    # Wait for consumer with timeout
    try:
        result = await asyncio.wait_for(consumer_task, timeout=2.0)
        assert result is not None
        assert "job.created" in result
        assert "test-123" in result
    except TimeoutError:
        consumer_task.cancel()
        pytest.fail("Timeout waiting for SSE event")


@pytest.mark.asyncio
async def test_event_stream_receives_multiple_event_types() -> None:
    """Test that event_stream receives multiple event types."""
    from submate.server.handlers.events.router import event_stream

    bus = get_event_bus()
    bus._subscribers.clear()

    stream = event_stream()
    events_received: list[str] = []

    async def consume_events() -> None:
        async for line in stream:
            events_received.append(line)
            if len(events_received) >= 3:
                return

    consumer_task = asyncio.create_task(consume_events())
    await asyncio.sleep(0.1)

    # Publish different event types
    bus.publish("job.created", {"job_id": "job-1"})
    bus.publish("job.started", {"job_id": "job-1"})
    bus.publish("job.completed", {"job_id": "job-1"})

    try:
        await asyncio.wait_for(consumer_task, timeout=2.0)
    except TimeoutError:
        consumer_task.cancel()
        pytest.fail("Timeout waiting for SSE events")

    # Each event produces one string with event: and data: lines
    assert len(events_received) == 3
    assert any("job.created" in e for e in events_received)
    assert any("job.started" in e for e in events_received)
    assert any("job.completed" in e for e in events_received)


@pytest.mark.asyncio
async def test_event_stream_sync_completed() -> None:
    """Test that sync.completed event is received via event_stream."""
    from submate.server.handlers.events.router import event_stream

    bus = get_event_bus()
    bus._subscribers.clear()

    stream = event_stream()

    async def consume_one() -> str | None:
        async for line in stream:
            return line
        return None

    consumer_task = asyncio.create_task(consume_one())
    await asyncio.sleep(0.1)

    bus.publish("sync.completed", {"libraries": 2, "items": 100})

    try:
        result = await asyncio.wait_for(consumer_task, timeout=2.0)
        assert result is not None
        assert "sync.completed" in result
        # Parse the data line
        for line in result.split("\n"):
            if line.startswith("data:"):
                data = json.loads(line[5:].strip())
                assert data["event_type"] == "sync.completed"
                assert data["data"]["libraries"] == 2
                assert data["data"]["items"] == 100
    except TimeoutError:
        consumer_task.cancel()
        pytest.fail("Timeout waiting for sync.completed event")


@pytest.mark.asyncio
async def test_event_stream_job_failed() -> None:
    """Test that job.failed event is received via event_stream."""
    from submate.server.handlers.events.router import event_stream

    bus = get_event_bus()
    bus._subscribers.clear()

    stream = event_stream()

    async def consume_one() -> str | None:
        async for line in stream:
            return line
        return None

    consumer_task = asyncio.create_task(consume_one())
    await asyncio.sleep(0.1)

    bus.publish("job.failed", {"job_id": "job-1", "error": "Transcription failed"})

    try:
        result = await asyncio.wait_for(consumer_task, timeout=2.0)
        assert result is not None
        assert "job.failed" in result
        for line in result.split("\n"):
            if line.startswith("data:"):
                data = json.loads(line[5:].strip())
                assert data["event_type"] == "job.failed"
                assert data["data"]["job_id"] == "job-1"
                assert data["data"]["error"] == "Transcription failed"
    except TimeoutError:
        consumer_task.cancel()
        pytest.fail("Timeout waiting for job.failed event")


def test_sse_event_types_defined() -> None:
    """Test that SSE_EVENT_TYPES includes all expected event types."""
    from submate.server.handlers.events.router import SSE_EVENT_TYPES

    expected_types = [
        "job.created",
        "job.started",
        "job.completed",
        "job.failed",
        "sync.completed",
    ]

    for event_type in expected_types:
        assert event_type in SSE_EVENT_TYPES
