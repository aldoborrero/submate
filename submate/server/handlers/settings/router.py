"""Settings API router for Submate UI."""

import logging
from pathlib import Path
from typing import Any

import requests
from fastapi import APIRouter

from submate.config import load_yaml_config, save_yaml_config
from submate.server.handlers.settings.models import (
    JellyfinSettings,
    NotificationSettings,
    SettingsResponse,
    SettingsUpdateRequest,
    TestConnectionResponse,
    TranslationSettings,
    WhisperSettings,
)

logger = logging.getLogger(__name__)


def _get_default_config_path() -> Path:
    """Get default configuration file path.

    Returns:
        Path to ~/.config/submate/config.yaml
    """
    return Path.home() / ".config" / "submate" / "config.yaml"


def _get_config_path() -> Path:
    """Get configuration file path.

    This function can be mocked in tests.

    Returns:
        Path to configuration file.
    """
    return _get_default_config_path()


def _deep_merge(base: dict[str, Any], update: dict[str, Any]) -> dict[str, Any]:
    """Deep merge two dictionaries.

    Args:
        base: Base dictionary
        update: Dictionary with updates to apply

    Returns:
        Merged dictionary
    """
    result = base.copy()
    for key, value in update.items():
        if key in result and isinstance(result[key], dict) and isinstance(value, dict):
            result[key] = _deep_merge(result[key], value)
        else:
            result[key] = value
    return result


def create_settings_router(config_path: Path | None = None) -> APIRouter:
    """Create settings API router.

    Args:
        config_path: Optional path to configuration file.
                    If not provided, uses default ~/.config/submate/config.yaml

    Returns:
        APIRouter with settings endpoints.
    """
    router = APIRouter(prefix="/api/settings", tags=["settings"])

    @router.get("", response_model=SettingsResponse)
    async def get_settings() -> SettingsResponse:
        """Get current settings from YAML configuration.

        Returns settings merged with defaults for any missing fields.
        """
        yaml_path = _get_config_path()
        config_data = load_yaml_config(yaml_path)

        # Build response from YAML data with defaults
        jellyfin_data = config_data.get("jellyfin", {})
        whisper_data = config_data.get("whisper", {})
        translation_data = config_data.get("translation", {})
        notifications_data = config_data.get("notifications", {})

        return SettingsResponse(
            jellyfin=JellyfinSettings(**jellyfin_data),
            whisper=WhisperSettings(**whisper_data),
            translation=TranslationSettings(**translation_data),
            notifications=NotificationSettings(**notifications_data),
        )

    @router.put("", response_model=SettingsResponse)
    async def update_settings(update: SettingsUpdateRequest) -> SettingsResponse:
        """Update settings and save to YAML file.

        Only fields provided in the request are updated.
        Existing fields are preserved.
        """
        yaml_path = _get_config_path()

        # Load existing config
        existing_config = load_yaml_config(yaml_path)

        # Build update dict from non-None fields
        update_dict: dict[str, Any] = {}
        if update.jellyfin is not None:
            update_dict["jellyfin"] = update.jellyfin.model_dump()
        if update.whisper is not None:
            update_dict["whisper"] = update.whisper.model_dump()
        if update.translation is not None:
            update_dict["translation"] = update.translation.model_dump()
        if update.notifications is not None:
            update_dict["notifications"] = update.notifications.model_dump()

        # Merge with existing config
        merged_config = _deep_merge(existing_config, update_dict)

        # Save to YAML
        save_yaml_config(yaml_path, merged_config)

        # Return updated settings
        return SettingsResponse(
            jellyfin=JellyfinSettings(**merged_config.get("jellyfin", {})),
            whisper=WhisperSettings(**merged_config.get("whisper", {})),
            translation=TranslationSettings(**merged_config.get("translation", {})),
            notifications=NotificationSettings(**merged_config.get("notifications", {})),
        )

    @router.post("/test-jellyfin", response_model=TestConnectionResponse)
    async def test_jellyfin_connection(settings: JellyfinSettings) -> TestConnectionResponse:
        """Test Jellyfin connection with provided settings.

        Attempts to connect to the Jellyfin server and fetch libraries.
        """
        # Validate required fields
        if not settings.server_url or not settings.api_key:
            return TestConnectionResponse(
                success=False,
                message="Server URL and API key are required",
                details={},
            )

        try:
            # Test connection by fetching libraries
            headers = {"X-MediaBrowser-Token": settings.api_key}
            response = requests.get(
                f"{settings.server_url.rstrip('/')}/Library/VirtualFolders",
                headers=headers,
                timeout=10,
            )
            response.raise_for_status()

            libraries = response.json()
            library_names = [lib.get("Name", "Unknown") for lib in libraries]

            return TestConnectionResponse(
                success=True,
                message=f"Connected successfully. Found {len(libraries)} libraries.",
                details={"libraries": library_names},
            )

        except requests.exceptions.ConnectionError as e:
            logger.warning("Jellyfin connection error: %s", e)
            return TestConnectionResponse(
                success=False,
                message=f"Connection error: Unable to reach server at {settings.server_url}",
                details={"error": str(e)},
            )

        except requests.exceptions.Timeout:
            return TestConnectionResponse(
                success=False,
                message="Connection timed out",
                details={},
            )

        except requests.exceptions.HTTPError as e:
            status_code = e.response.status_code if e.response is not None else "unknown"
            if status_code == 401:
                message = "Authentication failed: Invalid API key"
            elif status_code == 404:
                message = "Server not found: Check URL"
            else:
                message = f"HTTP error: {status_code}"
            return TestConnectionResponse(
                success=False,
                message=message,
                details={"status_code": status_code},
            )

        except Exception as e:
            logger.error("Unexpected error testing Jellyfin: %s", e, exc_info=True)
            return TestConnectionResponse(
                success=False,
                message=f"Unexpected error: {e}",
                details={"error": str(e)},
            )

    @router.post("/test-notification", response_model=TestConnectionResponse)
    async def test_notification(settings: NotificationSettings) -> TestConnectionResponse:
        """Test notification configuration.

        Sends a test notification via the configured channel.
        """
        # Check if any notification method is configured
        has_webhook = bool(settings.webhook_url)
        has_ntfy = bool(settings.ntfy_url and settings.ntfy_topic)
        has_apprise = bool(settings.apprise_urls)

        if not (has_webhook or has_ntfy or has_apprise):
            return TestConnectionResponse(
                success=False,
                message="No notification method configured",
                details={},
            )

        # Try webhook first
        if has_webhook:
            try:
                response = requests.post(
                    settings.webhook_url,  # type: ignore[arg-type]
                    json={
                        "text": "Test notification from Submate",
                        "event": "test",
                    },
                    timeout=10,
                )
                response.raise_for_status()
                return TestConnectionResponse(
                    success=True,
                    message="Webhook notification sent successfully",
                    details={"method": "webhook"},
                )
            except requests.exceptions.RequestException as e:
                logger.warning("Webhook notification failed: %s", e)
                return TestConnectionResponse(
                    success=False,
                    message=f"Webhook failed: {e}",
                    details={"method": "webhook", "error": str(e)},
                )

        # Try ntfy
        if has_ntfy:
            try:
                ntfy_url = f"{settings.ntfy_url.rstrip('/')}/{settings.ntfy_topic}"  # type: ignore[union-attr]
                response = requests.post(
                    ntfy_url,
                    data="Test notification from Submate",
                    headers={"Title": "Submate Test"},
                    timeout=10,
                )
                response.raise_for_status()
                return TestConnectionResponse(
                    success=True,
                    message="ntfy notification sent successfully",
                    details={"method": "ntfy"},
                )
            except requests.exceptions.RequestException as e:
                logger.warning("ntfy notification failed: %s", e)
                return TestConnectionResponse(
                    success=False,
                    message=f"ntfy failed: {e}",
                    details={"method": "ntfy", "error": str(e)},
                )

        # Try apprise (just validate URLs exist for now)
        if has_apprise:
            return TestConnectionResponse(
                success=True,
                message="Apprise URLs configured (test send not implemented)",
                details={"method": "apprise", "url_count": len(settings.apprise_urls)},
            )

        return TestConnectionResponse(
            success=False,
            message="No notification method could be tested",
            details={},
        )

    return router
