"""Settings API handlers for Submate UI."""

from submate.server.handlers.settings.models import (
    JellyfinSettings,
    NotificationSettings,
    SettingsResponse,
    SettingsUpdateRequest,
    TestConnectionResponse,
    TranslationSettings,
    WhisperSettings,
)
from submate.server.handlers.settings.router import create_settings_router

__all__ = [
    "JellyfinSettings",
    "NotificationSettings",
    "SettingsResponse",
    "SettingsUpdateRequest",
    "TestConnectionResponse",
    "TranslationSettings",
    "WhisperSettings",
    "create_settings_router",
]
