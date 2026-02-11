# tests/test_event_bus.py
"""Tests for the event bus service."""

from datetime import UTC, datetime


def test_event_bus_subscribe_and_publish():
    """Test subscribing and publishing events."""
    from submate.services.event_bus import EventBus

    bus = EventBus()
    received_events: list[dict] = []

    def handler(data: dict) -> None:
        received_events.append(data)

    # Subscribe to an event type
    sub_id = bus.subscribe("test_event", handler)
    assert sub_id is not None
    assert len(sub_id) > 0

    # Publish an event
    bus.publish("test_event", {"message": "hello"})

    # Verify handler received the event
    assert len(received_events) == 1
    assert received_events[0]["message"] == "hello"


def test_event_bus_multiple_subscribers():
    """Test multiple subscribers receive events."""
    from submate.services.event_bus import EventBus

    bus = EventBus()
    results_1: list[dict] = []
    results_2: list[dict] = []

    def handler_1(data: dict) -> None:
        results_1.append(data)

    def handler_2(data: dict) -> None:
        results_2.append(data)

    # Subscribe both handlers to same event type
    bus.subscribe("multi_event", handler_1)
    bus.subscribe("multi_event", handler_2)

    # Publish one event
    bus.publish("multi_event", {"value": 42})

    # Both handlers should receive the event
    assert len(results_1) == 1
    assert len(results_2) == 1
    assert results_1[0]["value"] == 42
    assert results_2[0]["value"] == 42


def test_event_bus_unsubscribe():
    """Test unsubscribing stops events."""
    from submate.services.event_bus import EventBus

    bus = EventBus()
    received: list[dict] = []

    def handler(data: dict) -> None:
        received.append(data)

    # Subscribe and capture the ID
    sub_id = bus.subscribe("unsub_event", handler)

    # First event should be received
    bus.publish("unsub_event", {"seq": 1})
    assert len(received) == 1

    # Unsubscribe
    result = bus.unsubscribe("unsub_event", sub_id)
    assert result is True

    # Second event should NOT be received
    bus.publish("unsub_event", {"seq": 2})
    assert len(received) == 1  # Still 1, not 2

    # Unsubscribing again should return False
    result = bus.unsubscribe("unsub_event", sub_id)
    assert result is False

    # Unsubscribing from non-existent event type should return False
    result = bus.unsubscribe("nonexistent", "fake-id")
    assert result is False


def test_event_bus_global_instance():
    """Test get_event_bus returns same instance."""
    from submate.services.event_bus import get_event_bus

    bus1 = get_event_bus()
    bus2 = get_event_bus()

    # Should be the exact same object (singleton)
    assert bus1 is bus2


def test_event_dataclass():
    """Test Event dataclass creation."""
    from submate.services.event_bus import Event

    now = datetime.now(UTC)
    event = Event(type="test", data={"key": "value"}, timestamp=now)

    assert event.type == "test"
    assert event.data == {"key": "value"}
    assert event.timestamp == now


def test_event_bus_handler_exception_does_not_break_others():
    """Test that one handler's exception doesn't prevent others from running."""
    from submate.services.event_bus import EventBus

    bus = EventBus()
    results: list[str] = []

    def bad_handler(data: dict) -> None:
        raise RuntimeError("Handler error")

    def good_handler(data: dict) -> None:
        results.append("success")

    # Subscribe both - bad handler first
    bus.subscribe("error_event", bad_handler)
    bus.subscribe("error_event", good_handler)

    # Publish - should not raise, good handler should still run
    bus.publish("error_event", {})

    assert len(results) == 1
    assert results[0] == "success"


def test_event_bus_different_event_types():
    """Test that events only go to subscribers of that type."""
    from submate.services.event_bus import EventBus

    bus = EventBus()
    type_a_events: list[dict] = []
    type_b_events: list[dict] = []

    def handler_a(data: dict) -> None:
        type_a_events.append(data)

    def handler_b(data: dict) -> None:
        type_b_events.append(data)

    bus.subscribe("type_a", handler_a)
    bus.subscribe("type_b", handler_b)

    bus.publish("type_a", {"from": "a"})
    bus.publish("type_b", {"from": "b"})

    assert len(type_a_events) == 1
    assert type_a_events[0]["from"] == "a"
    assert len(type_b_events) == 1
    assert type_b_events[0]["from"] == "b"
