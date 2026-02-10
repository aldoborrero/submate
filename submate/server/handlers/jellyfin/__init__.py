"""Jellyfin webhook integration."""

from submate.server.handlers.jellyfin.handlers import handle_jellyfin_webhook
from submate.server.handlers.jellyfin.models import JellyfinWebhookPayload

__all__ = ["handle_jellyfin_webhook", "JellyfinWebhookPayload"]
