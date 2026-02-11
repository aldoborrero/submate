"""Tests for extended Jellyfin client methods - library browsing support."""

from unittest.mock import Mock

import pytest

from submate.config import Config
from submate.media_servers.jellyfin import JellyfinClient


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
def connected_client(jellyfin_config: Config) -> JellyfinClient:
    """Provide a connected Jellyfin client for testing."""
    client = JellyfinClient(jellyfin_config)
    client.server_url = "http://localhost:8096"
    client.api_key = "fake-api-key"
    client._admin_user_id = "admin123"
    return client


@pytest.fixture
def disconnected_client(jellyfin_config: Config) -> JellyfinClient:
    """Provide a disconnected Jellyfin client for testing."""
    client = JellyfinClient(jellyfin_config)
    # server_url and api_key remain None
    return client


# get_libraries tests


def test_get_libraries(connected_client: JellyfinClient, mocker) -> None:
    """Test fetching libraries from Jellyfin."""
    mock_get = mocker.patch("submate.media_servers.jellyfin.requests.get")
    mock_response = Mock()
    mock_response.status_code = 200
    mock_response.json.return_value = [
        {"Id": "lib-1", "Name": "Movies", "CollectionType": "movies"},
        {"Id": "lib-2", "Name": "TV Shows", "CollectionType": "tvshows"},
        {"Id": "lib-3", "Name": "Music", "CollectionType": "music"},
    ]
    mock_get.return_value = mock_response

    libraries = connected_client.get_libraries()

    assert len(libraries) == 3
    assert libraries[0]["Id"] == "lib-1"
    assert libraries[0]["Name"] == "Movies"
    assert libraries[1]["Name"] == "TV Shows"
    mock_get.assert_called_once()
    call_args = mock_get.call_args
    assert "/Library/VirtualFolders" in call_args[0][0]
    assert call_args[1]["headers"]["X-MediaBrowser-Token"] == "fake-api-key"


def test_get_libraries_not_connected(disconnected_client: JellyfinClient) -> None:
    """Test get_libraries raises error when not connected."""
    with pytest.raises(RuntimeError, match="Not connected"):
        disconnected_client.get_libraries()


# get_library_items tests


def test_get_library_items(connected_client: JellyfinClient, mocker) -> None:
    """Test fetching items from a library."""
    mock_get = mocker.patch("submate.media_servers.jellyfin.requests.get")
    mock_response = Mock()
    mock_response.status_code = 200
    mock_response.json.return_value = {
        "Items": [
            {"Id": "movie-1", "Name": "Test Movie 1", "Type": "Movie"},
            {"Id": "movie-2", "Name": "Test Movie 2", "Type": "Movie"},
        ],
        "TotalRecordCount": 150,
    }
    mock_get.return_value = mock_response

    result = connected_client.get_library_items("lib-1", item_type="Movie", start_index=0, limit=100)

    assert "Items" in result
    assert "TotalRecordCount" in result
    assert len(result["Items"]) == 2
    assert result["TotalRecordCount"] == 150
    mock_get.assert_called_once()
    call_args = mock_get.call_args
    assert "/Users/admin123/Items" in call_args[0][0]
    assert call_args[1]["params"]["ParentId"] == "lib-1"
    assert call_args[1]["params"]["IncludeItemTypes"] == "Movie"
    assert call_args[1]["params"]["Recursive"] == "true"


def test_get_library_items_with_pagination(connected_client: JellyfinClient, mocker) -> None:
    """Test fetching library items with custom pagination."""
    mock_get = mocker.patch("submate.media_servers.jellyfin.requests.get")
    mock_response = Mock()
    mock_response.status_code = 200
    mock_response.json.return_value = {"Items": [], "TotalRecordCount": 150}
    mock_get.return_value = mock_response

    connected_client.get_library_items("lib-1", start_index=50, limit=25)

    call_args = mock_get.call_args
    assert call_args[1]["params"]["StartIndex"] == 50
    assert call_args[1]["params"]["Limit"] == 25


def test_get_library_items_not_connected(disconnected_client: JellyfinClient) -> None:
    """Test get_library_items raises error when not connected."""
    with pytest.raises(RuntimeError, match="Not connected"):
        disconnected_client.get_library_items("lib-1")


# get_series_episodes tests


def test_get_series_episodes(connected_client: JellyfinClient, mocker) -> None:
    """Test fetching episodes for a series."""
    mock_get = mocker.patch("submate.media_servers.jellyfin.requests.get")
    mock_response = Mock()
    mock_response.status_code = 200
    mock_response.json.return_value = {
        "Items": [
            {"Id": "ep-1", "Name": "Pilot", "IndexNumber": 1, "ParentIndexNumber": 1},
            {"Id": "ep-2", "Name": "Episode 2", "IndexNumber": 2, "ParentIndexNumber": 1},
        ],
        "TotalRecordCount": 24,
    }
    mock_get.return_value = mock_response

    result = connected_client.get_series_episodes("series-123")

    assert "Items" in result
    assert "TotalRecordCount" in result
    assert len(result["Items"]) == 2
    assert result["Items"][0]["Name"] == "Pilot"
    mock_get.assert_called_once()
    call_args = mock_get.call_args
    assert "/Shows/series-123/Episodes" in call_args[0][0]


def test_get_series_episodes_not_connected(disconnected_client: JellyfinClient) -> None:
    """Test get_series_episodes raises error when not connected."""
    with pytest.raises(RuntimeError, match="Not connected"):
        disconnected_client.get_series_episodes("series-123")


# get_item tests


def test_get_item(connected_client: JellyfinClient, mocker) -> None:
    """Test fetching a single item by ID."""
    mock_get = mocker.patch("submate.media_servers.jellyfin.requests.get")
    mock_response = Mock()
    mock_response.status_code = 200
    mock_response.json.return_value = {
        "Id": "item-456",
        "Name": "Test Movie",
        "Type": "Movie",
        "Path": "/media/movies/test.mkv",
        "RunTimeTicks": 72000000000,
    }
    mock_get.return_value = mock_response

    result = connected_client.get_item("item-456")

    assert result["Id"] == "item-456"
    assert result["Name"] == "Test Movie"
    assert result["Type"] == "Movie"
    mock_get.assert_called_once()
    call_args = mock_get.call_args
    assert "/Users/admin123/Items/item-456" in call_args[0][0]


def test_get_item_not_connected(disconnected_client: JellyfinClient) -> None:
    """Test get_item raises error when not connected."""
    with pytest.raises(RuntimeError, match="Not connected"):
        disconnected_client.get_item("item-456")


# get_poster_url tests


def test_get_poster_url(connected_client: JellyfinClient) -> None:
    """Test generating poster URL for an item."""
    url = connected_client.get_poster_url("item-789")

    assert url == "http://localhost:8096/Items/item-789/Images/Primary"


def test_get_poster_url_not_connected(disconnected_client: JellyfinClient) -> None:
    """Test get_poster_url raises error when not connected."""
    with pytest.raises(RuntimeError, match="Not connected"):
        disconnected_client.get_poster_url("item-789")
