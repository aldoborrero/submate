"""Tests for Jellyfin integration - client, handlers, and models."""

from unittest.mock import Mock, patch

import pytest
from pydantic import ValidationError

from submate.config import Config
from submate.media_servers.jellyfin import JellyfinClient
from submate.server.handlers.jellyfin import JellyfinWebhookPayload, handle_jellyfin_webhook

# Fixtures


@pytest.fixture
def jellyfin_config() -> Config:
    """Provide Jellyfin configuration."""
    return Config(
        jellyfin={
            "server_url": "http://localhost:8096",
            "api_key": "fake-api-key",
            "libraries": ["Movies", "TV Shows"],
        }
    )


@pytest.fixture
def mock_jellyfin_response() -> Mock:
    """Provide a mock response for Jellyfin API."""
    response = Mock()
    response.status_code = 200
    response.json.return_value = [
        {"Name": "Movies", "Id": "library-1"},
        {"Name": "TV Shows", "Id": "library-2"},
    ]
    return response


@pytest.fixture
def mock_jellyfin_client(jellyfin_config: Config) -> JellyfinClient:
    """Provide a connected Jellyfin client for testing."""
    client = JellyfinClient(jellyfin_config)
    client.server_url = "http://localhost:8096"
    client.api_key = "fake-api-key"
    return client


@pytest.fixture
def jellyfin_payload():
    """Sample Jellyfin webhook payload."""
    return JellyfinWebhookPayload(
        notification_type="ItemAdded",
        item_id="item123",
        item_type="Episode",
        name="Test Episode",
    )


# Client tests


def test_client_not_configured() -> None:
    """Test client when Jellyfin is not configured."""
    config = Config(jellyfin={"server_url": "", "api_key": ""})
    client = JellyfinClient(config)
    assert client.is_configured() is False


def test_client_is_configured(jellyfin_config: Config) -> None:
    """Test client when Jellyfin is configured."""
    client = JellyfinClient(jellyfin_config)
    assert client.is_configured() is True


@patch("submate.media_servers.jellyfin.requests.get")
def test_client_connect(mock_get: Mock, jellyfin_config: Config, mock_jellyfin_response: Mock) -> None:
    """Test connecting to Jellyfin server."""
    mock_get.return_value = mock_jellyfin_response
    client = JellyfinClient(jellyfin_config)

    client.connect()

    assert client.server_url == "http://localhost:8096"
    assert client.api_key == "fake-api-key"


@patch("submate.media_servers.jellyfin.requests.post")
@patch("submate.media_servers.jellyfin.requests.get")
def test_client_refresh_library(
    mock_get: Mock, mock_post: Mock, jellyfin_config: Config, mock_jellyfin_response: Mock
) -> None:
    """Test refreshing a Jellyfin library."""
    mock_get.return_value = mock_jellyfin_response
    mock_post.return_value = Mock(status_code=204)

    client = JellyfinClient(jellyfin_config)
    client.connect()
    client.refresh_library("Movies")

    mock_post.assert_called_once()


@patch("submate.media_servers.jellyfin.requests.post")
@patch("submate.media_servers.jellyfin.requests.get")
def test_client_refresh_all_libraries(
    mock_get: Mock, mock_post: Mock, jellyfin_config: Config, mock_jellyfin_response: Mock
) -> None:
    """Test refreshing all configured libraries."""
    mock_get.return_value = mock_jellyfin_response
    mock_post.return_value = Mock(status_code=204)

    client = JellyfinClient(jellyfin_config)
    client.connect()
    refreshed = client.refresh_all_libraries()

    assert len(refreshed) > 0


def test_client_get_file_path(mock_jellyfin_client, mocker):
    """Test getting file path from item ID."""
    mock_get = mocker.patch("requests.get")

    admin_response = mocker.MagicMock()
    admin_response.status_code = 200
    admin_response.json.return_value = [{"Id": "admin123", "Policy": {"IsAdministrator": True}}]

    item_response = mocker.MagicMock()
    item_response.status_code = 200
    item_response.json.return_value = {"Path": "/media/movies/test.mkv"}

    mock_get.side_effect = [admin_response, item_response]

    file_path = mock_jellyfin_client.get_file_path("item456")
    assert file_path == "/media/movies/test.mkv"


def test_client_refresh_item(mock_jellyfin_client, mocker):
    """Test refreshing item metadata."""
    mock_post = mocker.patch("requests.post")
    mock_post.return_value = mocker.MagicMock(status_code=204)

    mock_jellyfin_client.refresh_item("item123")

    mock_post.assert_called_once()


# Webhook handler tests


@pytest.mark.asyncio
async def test_webhook_item_added(jellyfin_payload, mocker):
    """Test handling Jellyfin ItemAdded event."""
    mock_client = mocker.patch("submate.server.handlers.jellyfin.handlers.JellyfinClient")
    mock_task_queue = mocker.MagicMock()
    mocker.patch("submate.server.handlers.jellyfin.handlers.get_task_queue", return_value=mock_task_queue)
    mock_config = mocker.patch("submate.server.handlers.jellyfin.handlers.get_config")

    mock_config.return_value.jellyfin.server_url = "http://localhost:8096"
    mock_config.return_value.jellyfin.api_key = "test-key"
    mock_config.return_value.path_mapping.enabled = False
    mock_client.return_value.get_file_path.return_value = "/media/test.mkv"

    result = await handle_jellyfin_webhook(jellyfin_payload)

    assert result["status"] == "queued"
    mock_task_queue.enqueue.assert_called_once()


@pytest.mark.asyncio
async def test_webhook_skip_event(jellyfin_payload, mocker):
    """Test skipping non-relevant Jellyfin events."""
    jellyfin_payload.notification_type = "UserDataSaved"

    result = await handle_jellyfin_webhook(jellyfin_payload)

    assert result["status"] == "skipped"


@pytest.mark.asyncio
async def test_webhook_path_mapping(jellyfin_payload, mocker):
    """Test path mapping for Docker deployments."""
    mock_client = mocker.patch("submate.server.handlers.jellyfin.handlers.JellyfinClient")
    mock_task_queue = mocker.MagicMock()
    mocker.patch("submate.server.handlers.jellyfin.handlers.get_task_queue", return_value=mock_task_queue)
    mock_config = mocker.patch("submate.server.handlers.jellyfin.handlers.get_config")

    mock_config.return_value.path_mapping.enabled = True
    mock_config.return_value.path_mapping.from_path = "/host/media"
    mock_config.return_value.path_mapping.to_path = "/container/media"
    mock_client.return_value.get_file_path.return_value = "/host/media/test.mkv"

    await handle_jellyfin_webhook(jellyfin_payload)

    mock_task_queue.enqueue.assert_called_once()
    assert "/container/media/test.mkv" in str(mock_task_queue.enqueue.call_args)


# Webhook payload model tests


def test_payload_valid():
    """Test valid Jellyfin webhook payload."""
    payload = {
        "NotificationType": "ItemAdded",
        "ItemId": "item123",
        "ItemType": "Episode",
        "Name": "Test Episode",
        "ServerId": "server456",
    }

    webhook = JellyfinWebhookPayload.model_validate(payload)

    assert webhook.notification_type == "ItemAdded"
    assert webhook.item_id == "item123"


def test_payload_missing_fields():
    """Test invalid Jellyfin webhook payload."""
    with pytest.raises(ValidationError):
        JellyfinWebhookPayload.model_validate({"NotificationType": "ItemAdded"})


def test_payload_event_types():
    """Test event type detection methods."""
    added = JellyfinWebhookPayload(notification_type="ItemAdded", item_id="test")
    playback = JellyfinWebhookPayload(notification_type="PlaybackStart", item_id="test")

    assert added.is_item_added()
    assert not added.is_playback_start()
    assert playback.is_playback_start()
    assert not playback.is_item_added()
