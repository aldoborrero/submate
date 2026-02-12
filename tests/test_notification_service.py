"""Tests for the notification service."""

from pathlib import Path
from typing import Any
from unittest.mock import MagicMock, patch


class TestNotificationService:
    """Tests for NotificationService."""

    def test_notification_service_subscribe_to_events(self) -> None:
        """Verify service subscribes to job events on start."""
        from submate.services.event_bus import EventBus
        from submate.services.notifications import NotificationService

        event_bus = EventBus()
        service = NotificationService(event_bus=event_bus)

        # Before start, no subscriptions
        assert len(service._subscriptions) == 0

        # Start the service
        service.start()

        # After start, should have subscriptions for job.completed and job.failed
        assert len(service._subscriptions) == 2

        # Verify the subscriptions exist by checking they are in the event bus
        assert len(event_bus._subscribers.get("job.completed", {})) == 1
        assert len(event_bus._subscribers.get("job.failed", {})) == 1

        # Clean up
        service.stop()

    def test_notification_service_send_webhook(self, tmp_path: Path) -> None:
        """Test webhook notification sending."""
        from submate.services.notifications import NotificationService

        # Create a config file with webhook URL
        config_path = tmp_path / "config.yaml"
        config_path.write_text(
            """
notifications:
  webhook_url: "https://example.com/webhook"
"""
        )

        service = NotificationService(config_path=config_path)

        with patch("requests.post") as mock_post:
            mock_response = MagicMock()
            mock_response.raise_for_status = MagicMock()
            mock_post.return_value = mock_response

            results = service.send_notification(
                title="Test Title",
                message="Test message",
                data={"job_id": "123"},
            )

            assert results["webhook"] is True
            mock_post.assert_called_once()

            # Verify the payload
            call_args = mock_post.call_args
            assert call_args.kwargs["json"]["title"] == "Test Title"
            assert call_args.kwargs["json"]["message"] == "Test message"
            assert call_args.kwargs["json"]["data"]["job_id"] == "123"

    def test_notification_service_send_webhook_failure(self, tmp_path: Path) -> None:
        """Test webhook notification handles failures gracefully."""
        from submate.services.notifications import NotificationService

        config_path = tmp_path / "config.yaml"
        config_path.write_text(
            """
notifications:
  webhook_url: "https://example.com/webhook"
"""
        )

        service = NotificationService(config_path=config_path)

        with patch("requests.post") as mock_post:
            mock_post.side_effect = Exception("Connection error")

            results = service.send_notification(
                title="Test",
                message="Test message",
            )

            assert results["webhook"] is False

    def test_notification_service_send_ntfy(self, tmp_path: Path) -> None:
        """Test ntfy notification sending."""
        from submate.services.notifications import NotificationService

        config_path = tmp_path / "config.yaml"
        config_path.write_text(
            """
notifications:
  ntfy_url: "https://ntfy.sh"
  ntfy_topic: "submate-test"
"""
        )

        service = NotificationService(config_path=config_path)

        with patch("requests.post") as mock_post:
            mock_response = MagicMock()
            mock_response.raise_for_status = MagicMock()
            mock_post.return_value = mock_response

            results = service.send_notification(
                title="Transcription Complete",
                message="Job finished successfully",
                priority="high",
            )

            assert results["ntfy"] is True
            mock_post.assert_called_once()

            # Verify the URL and headers
            call_args = mock_post.call_args
            assert call_args.args[0] == "https://ntfy.sh/submate-test"
            assert call_args.kwargs["headers"]["Title"] == "Transcription Complete"
            assert call_args.kwargs["headers"]["Priority"] == "high"

    def test_notification_service_send_ntfy_failure(self, tmp_path: Path) -> None:
        """Test ntfy notification handles failures gracefully."""
        from submate.services.notifications import NotificationService

        config_path = tmp_path / "config.yaml"
        config_path.write_text(
            """
notifications:
  ntfy_url: "https://ntfy.sh"
  ntfy_topic: "submate-test"
"""
        )

        service = NotificationService(config_path=config_path)

        with patch("requests.post") as mock_post:
            mock_post.side_effect = Exception("Connection error")

            results = service.send_notification(title="Test", message="Message")

            assert results["ntfy"] is False

    def test_notification_service_send_apprise(self, tmp_path: Path) -> None:
        """Test Apprise notification sending."""
        from submate.services.notifications import NotificationService

        config_path = tmp_path / "config.yaml"
        config_path.write_text(
            """
notifications:
  apprise_urls:
    - "mailto://user:pass@example.com"
    - "slack://token/channel"
"""
        )

        service = NotificationService(config_path=config_path)

        # Mock the apprise import and usage
        with patch.dict("sys.modules", {"apprise": MagicMock()}):
            import sys

            mock_apprise_module = sys.modules["apprise"]
            mock_apprise_instance = MagicMock()
            mock_apprise_instance.notify.return_value = True
            mock_apprise_module.Apprise.return_value = mock_apprise_instance

            results = service.send_notification(
                title="Test Title",
                message="Test message",
            )

            assert results["apprise"] is True
            mock_apprise_instance.add.assert_called()
            mock_apprise_instance.notify.assert_called_once_with(title="Test Title", body="Test message")

    def test_notification_service_send_apprise_not_installed(self, tmp_path: Path) -> None:
        """Test Apprise notification returns False when apprise is not installed."""
        from submate.services.notifications import NotificationService

        config_path = tmp_path / "config.yaml"
        config_path.write_text(
            """
notifications:
  apprise_urls:
    - "mailto://user:pass@example.com"
"""
        )

        service = NotificationService(config_path=config_path)

        # Simulate ImportError for apprise
        with patch("submate.services.notifications.NotificationService._send_apprise") as mock_send:
            mock_send.return_value = False

            results = service.send_notification(
                title="Test",
                message="Test message",
            )

            assert results["apprise"] is False

    def test_notification_service_on_job_completed(self, tmp_path: Path) -> None:
        """Test job completion triggers notification."""
        from submate.services.event_bus import EventBus
        from submate.services.notifications import NotificationService

        config_path = tmp_path / "config.yaml"
        config_path.write_text(
            """
notifications:
  webhook_url: "https://example.com/webhook"
"""
        )

        event_bus = EventBus()
        service = NotificationService(config_path=config_path, event_bus=event_bus)
        service.start()

        with patch.object(service, "send_notification") as mock_send:
            mock_send.return_value = {"webhook": True}

            # Publish a job.completed event
            event_bus.publish(
                "job.completed",
                {
                    "job_id": "job-123",
                    "item_title": "Movie Title",
                    "status": "completed",
                },
            )

            mock_send.assert_called_once()
            call_args = mock_send.call_args

            assert call_args.kwargs["title"] == "Transcription Complete"
            assert "job-123" in call_args.kwargs["message"]
            assert "Movie Title" in call_args.kwargs["message"]

        service.stop()

    def test_notification_service_on_job_failed(self, tmp_path: Path) -> None:
        """Test job failure triggers notification with high priority."""
        from submate.services.event_bus import EventBus
        from submate.services.notifications import NotificationService

        config_path = tmp_path / "config.yaml"
        config_path.write_text(
            """
notifications:
  ntfy_url: "https://ntfy.sh"
  ntfy_topic: "submate"
"""
        )

        event_bus = EventBus()
        service = NotificationService(config_path=config_path, event_bus=event_bus)
        service.start()

        with patch.object(service, "send_notification") as mock_send:
            mock_send.return_value = {"ntfy": True}

            # Publish a job.failed event
            event_bus.publish(
                "job.failed",
                {
                    "job_id": "job-456",
                    "error": "Transcription timeout",
                },
            )

            mock_send.assert_called_once()
            call_args = mock_send.call_args

            assert call_args.kwargs["title"] == "Transcription Failed"
            assert "job-456" in call_args.kwargs["message"]
            assert "Transcription timeout" in call_args.kwargs["message"]
            assert call_args.kwargs["priority"] == "high"

        service.stop()

    def test_notification_service_no_config(self, tmp_path: Path) -> None:
        """Test returns empty results when no notifications configured."""
        from submate.services.notifications import NotificationService

        # Create an empty config file
        config_path = tmp_path / "config.yaml"
        config_path.write_text(
            """
# Empty notifications config
"""
        )

        service = NotificationService(config_path=config_path)

        results = service.send_notification(
            title="Test",
            message="Test message",
        )

        # No channels configured, so empty results
        assert results == {}

    def test_notification_service_config_file_not_exists(self, tmp_path: Path) -> None:
        """Test handles missing config file gracefully."""
        from submate.services.notifications import NotificationService

        # Point to non-existent config file
        config_path = tmp_path / "nonexistent.yaml"

        service = NotificationService(config_path=config_path)

        results = service.send_notification(
            title="Test",
            message="Test message",
        )

        # No config file, so empty results
        assert results == {}

    def test_notification_service_start_stop(self) -> None:
        """Test starting and stopping the service."""
        from submate.services.event_bus import EventBus
        from submate.services.notifications import NotificationService

        event_bus = EventBus()
        service = NotificationService(event_bus=event_bus)

        # Start the service
        service.start()
        assert len(service._subscriptions) == 2
        assert len(event_bus._subscribers.get("job.completed", {})) == 1
        assert len(event_bus._subscribers.get("job.failed", {})) == 1

        # Stop the service
        service.stop()
        assert len(service._subscriptions) == 0
        # Subscriptions should be removed
        assert len(event_bus._subscribers.get("job.completed", {})) == 0
        assert len(event_bus._subscribers.get("job.failed", {})) == 0

    def test_notification_service_multiple_channels(self, tmp_path: Path) -> None:
        """Test sending notifications through multiple channels."""
        from submate.services.notifications import NotificationService

        config_path = tmp_path / "config.yaml"
        config_path.write_text(
            """
notifications:
  webhook_url: "https://example.com/webhook"
  ntfy_url: "https://ntfy.sh"
  ntfy_topic: "submate"
"""
        )

        service = NotificationService(config_path=config_path)

        with patch("requests.post") as mock_post:
            mock_response = MagicMock()
            mock_response.raise_for_status = MagicMock()
            mock_post.return_value = mock_response

            results = service.send_notification(
                title="Test",
                message="Test message",
            )

            # Both channels should succeed
            assert results["webhook"] is True
            assert results["ntfy"] is True

            # Two POST requests should have been made
            assert mock_post.call_count == 2

    def test_notification_service_partial_failure(self, tmp_path: Path) -> None:
        """Test that failure in one channel doesn't affect others."""
        from submate.services.notifications import NotificationService

        config_path = tmp_path / "config.yaml"
        config_path.write_text(
            """
notifications:
  webhook_url: "https://example.com/webhook"
  ntfy_url: "https://ntfy.sh"
  ntfy_topic: "submate"
"""
        )

        service = NotificationService(config_path=config_path)

        call_count = 0

        def mock_post_side_effect(*args: Any, **kwargs: Any) -> MagicMock:
            nonlocal call_count
            call_count += 1
            if call_count == 1:
                # First call (webhook) fails
                raise Exception("Webhook failed")
            # Second call (ntfy) succeeds
            mock_response = MagicMock()
            mock_response.raise_for_status = MagicMock()
            return mock_response

        with patch("requests.post", side_effect=mock_post_side_effect):
            results = service.send_notification(
                title="Test",
                message="Test message",
            )

            # Webhook failed, ntfy succeeded
            assert results["webhook"] is False
            assert results["ntfy"] is True


class TestGetNotificationService:
    """Tests for get_notification_service singleton."""

    def test_get_notification_service_returns_same_instance(self) -> None:
        """Test that get_notification_service returns the same instance."""
        from submate.services import notifications

        # Reset the global instance
        notifications._notification_service = None

        service1 = notifications.get_notification_service()
        service2 = notifications.get_notification_service()

        assert service1 is service2

        # Clean up
        notifications._notification_service = None
