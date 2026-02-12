"""Notification service for sending alerts on job events."""

import logging
from pathlib import Path
from typing import Any

import requests

from submate.config import load_yaml_config
from submate.services.event_bus import EventBus, get_event_bus

logger = logging.getLogger(__name__)


class NotificationService:
    """Service for sending notifications on job events.

    This service subscribes to job events (job.completed, job.failed) and sends
    notifications through configured channels (webhook, ntfy, apprise).

    Example:
        service = NotificationService()
        service.start()  # Start listening for job events

        # Or send notifications directly
        service.send_notification(
            title="Job Complete",
            message="Transcription finished",
            data={"job_id": "123"},
        )

        service.stop()  # Stop listening
    """

    def __init__(
        self,
        config_path: Path | None = None,
        event_bus: EventBus | None = None,
    ) -> None:
        """Initialize notification service.

        Args:
            config_path: Path to YAML config file (default: ~/.config/submate/config.yaml)
            event_bus: Event bus to subscribe to (default: global instance)
        """
        self.config_path = config_path or Path.home() / ".config" / "submate" / "config.yaml"
        self.event_bus = event_bus or get_event_bus()
        self._subscriptions: list[tuple[str, str]] = []

    def start(self) -> None:
        """Start listening for job events.

        Subscribes to job.completed and job.failed events on the event bus.
        When events are received, notifications are sent through configured channels.
        """
        sub_id_completed = self.event_bus.subscribe("job.completed", self._on_job_completed)
        self._subscriptions.append(("job.completed", sub_id_completed))

        sub_id_failed = self.event_bus.subscribe("job.failed", self._on_job_failed)
        self._subscriptions.append(("job.failed", sub_id_failed))

        logger.info("Notification service started")

    def stop(self) -> None:
        """Stop listening for job events.

        Unsubscribes from all event types that were subscribed to during start().
        """
        for event_type, sub_id in self._subscriptions:
            self.event_bus.unsubscribe(event_type, sub_id)

        self._subscriptions.clear()
        logger.info("Notification service stopped")

    def _on_job_completed(self, data: dict[str, Any]) -> None:
        """Handle job completion event.

        Args:
            data: Event data containing job_id, item_title, etc.
        """
        title = "Transcription Complete"
        job_id = data.get("job_id", "unknown")
        item_title = data.get("item_title", "unknown")
        message = f"Job {job_id} completed for {item_title}"
        self.send_notification(title=title, message=message, data=data)

    def _on_job_failed(self, data: dict[str, Any]) -> None:
        """Handle job failure event.

        Args:
            data: Event data containing job_id, error, etc.
        """
        title = "Transcription Failed"
        job_id = data.get("job_id", "unknown")
        error = data.get("error", "unknown error")
        message = f"Job {job_id} failed: {error}"
        self.send_notification(title=title, message=message, data=data, priority="high")

    def send_notification(
        self,
        title: str,
        message: str,
        data: dict[str, Any] | None = None,
        priority: str = "default",
    ) -> dict[str, bool]:
        """Send notification via all configured channels.

        Args:
            title: Notification title
            message: Notification message
            data: Additional data to include in the notification payload
            priority: Notification priority (default, high, urgent)

        Returns:
            Dict mapping channel name to success status. Empty dict if no channels configured.
        """
        config = self._load_notification_config()
        results: dict[str, bool] = {}

        if config.get("webhook_url"):
            results["webhook"] = self._send_webhook(config["webhook_url"], title, message, data)

        if config.get("ntfy_url") and config.get("ntfy_topic"):
            results["ntfy"] = self._send_ntfy(config["ntfy_url"], config["ntfy_topic"], title, message, priority)

        if config.get("apprise_urls"):
            results["apprise"] = self._send_apprise(config["apprise_urls"], title, message)

        return results

    def _load_notification_config(self) -> dict[str, Any]:
        """Load notification settings from YAML config.

        Returns:
            Notification configuration dict, or empty dict if not configured.
        """
        config = load_yaml_config(self.config_path)
        notifications = config.get("notifications", {})
        if notifications is None:
            return {}
        return dict(notifications)

    def _send_webhook(
        self,
        url: str,
        title: str,
        message: str,
        data: dict[str, Any] | None = None,
    ) -> bool:
        """Send webhook notification.

        Args:
            url: Webhook URL to POST to
            title: Notification title
            message: Notification message
            data: Additional data to include

        Returns:
            True if successful, False otherwise.
        """
        try:
            payload = {
                "title": title,
                "message": message,
                "data": data or {},
            }
            response = requests.post(url, json=payload, timeout=10)
            response.raise_for_status()
            logger.info(f"Webhook notification sent to {url}")
            return True
        except Exception as e:
            logger.error(f"Failed to send webhook: {e}")
            return False

    def _send_ntfy(
        self,
        url: str,
        topic: str,
        title: str,
        message: str,
        priority: str = "default",
    ) -> bool:
        """Send ntfy notification.

        Args:
            url: ntfy server URL
            topic: ntfy topic name
            title: Notification title
            message: Notification message
            priority: Notification priority

        Returns:
            True if successful, False otherwise.
        """
        try:
            headers = {
                "Title": title,
                "Priority": priority,
            }
            response = requests.post(
                f"{url.rstrip('/')}/{topic}",
                data=message.encode("utf-8"),
                headers=headers,
                timeout=10,
            )
            response.raise_for_status()
            logger.info(f"ntfy notification sent to {topic}")
            return True
        except Exception as e:
            logger.error(f"Failed to send ntfy: {e}")
            return False

    def _send_apprise(
        self,
        urls: list[str],
        title: str,
        message: str,
    ) -> bool:
        """Send Apprise notifications.

        Uses the apprise library if available. Falls back gracefully if not installed.

        Args:
            urls: List of Apprise notification URLs
            title: Notification title
            message: Notification message

        Returns:
            True if successful, False if apprise is not installed or notification failed.
        """
        try:
            try:
                import apprise

                apobj = apprise.Apprise()
                for url in urls:
                    apobj.add(url)

                result = apobj.notify(title=title, body=message)
                logger.info(f"Apprise notification sent to {len(urls)} target(s)")
                return bool(result)
            except ImportError:
                logger.warning("Apprise library not installed, skipping apprise notifications")
                return False
        except Exception as e:
            logger.error(f"Failed to send Apprise notification: {e}")
            return False


# Global instance
_notification_service: NotificationService | None = None


def get_notification_service() -> NotificationService:
    """Get global notification service instance.

    Returns:
        The global NotificationService singleton instance.
    """
    global _notification_service
    if _notification_service is None:
        _notification_service = NotificationService()
    return _notification_service
