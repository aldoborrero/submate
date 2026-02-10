"""Pydantic models for Jellyfin webhook payloads."""

from pydantic import BaseModel, Field


class JellyfinWebhookPayload(BaseModel):
    """Jellyfin webhook notification payload.

    Sent by Jellyfin when configured webhook events occur.
    """

    notification_type: str = Field(alias="NotificationType")
    item_id: str = Field(alias="ItemId")
    item_type: str | None = Field(default=None, alias="ItemType")
    name: str | None = Field(default=None, alias="Name")
    server_id: str | None = Field(default=None, alias="ServerId")

    model_config = {"populate_by_name": True}

    def is_item_added(self) -> bool:
        """Check if this is an ItemAdded event."""
        return self.notification_type == "ItemAdded"

    def is_playback_start(self) -> bool:
        """Check if this is a PlaybackStart event."""
        return self.notification_type == "PlaybackStart"
