"""Tests for Settings API endpoints."""

from pathlib import Path

import pytest
from fastapi.testclient import TestClient

from submate.config import save_yaml_config
from submate.server import app


@pytest.fixture
def client():
    """FastAPI test client."""
    return TestClient(app)


@pytest.fixture
def config_path(tmp_path: Path) -> Path:
    """Create a temporary config file path."""
    return tmp_path / "config.yaml"


def test_get_settings_defaults(client: TestClient, config_path: Path, mocker):
    """Test GET /api/settings returns default settings when no config exists."""
    # Mock the config path helper to use our test config path
    mocker.patch(
        "submate.server.handlers.settings.router._get_config_path",
        return_value=config_path,
    )

    response = client.get("/api/settings")

    assert response.status_code == 200
    data = response.json()

    # Check default values
    assert "jellyfin" in data
    assert data["jellyfin"]["server_url"] == ""
    assert data["jellyfin"]["api_key"] == ""

    assert "whisper" in data
    assert data["whisper"]["model"] == "medium"
    assert data["whisper"]["device"] == "cpu"

    assert "translation" in data
    assert data["translation"]["backend"] == "ollama"
    assert data["translation"]["ollama_url"] == "http://localhost:11434"

    assert "notifications" in data
    assert data["notifications"]["webhook_url"] is None
    assert data["notifications"]["ntfy_url"] is None
    assert data["notifications"]["ntfy_topic"] is None
    assert data["notifications"]["apprise_urls"] == []


def test_get_settings_from_yaml(client: TestClient, config_path: Path, mocker):
    """Test GET /api/settings returns settings from YAML file."""
    # Create a config file with custom settings
    config_data = {
        "jellyfin": {
            "server_url": "http://jellyfin.local:8096",
            "api_key": "test-api-key",
        },
        "whisper": {
            "model": "large",
            "device": "cuda",
        },
        "translation": {
            "backend": "openai",
            "openai_api_key": "sk-test-key",
        },
    }
    save_yaml_config(config_path, config_data)

    mocker.patch(
        "submate.server.handlers.settings.router._get_config_path",
        return_value=config_path,
    )

    response = client.get("/api/settings")

    assert response.status_code == 200
    data = response.json()

    assert data["jellyfin"]["server_url"] == "http://jellyfin.local:8096"
    assert data["jellyfin"]["api_key"] == "test-api-key"
    assert data["whisper"]["model"] == "large"
    assert data["whisper"]["device"] == "cuda"
    assert data["translation"]["backend"] == "openai"


def test_update_settings_saves_yaml(client: TestClient, config_path: Path, mocker):
    """Test PUT /api/settings saves updated settings to YAML."""
    mocker.patch(
        "submate.server.handlers.settings.router._get_config_path",
        return_value=config_path,
    )

    update_data = {
        "jellyfin": {
            "server_url": "http://my-jellyfin:8096",
            "api_key": "my-api-key",
        },
        "whisper": {
            "model": "small",
            "device": "cpu",
        },
    }

    response = client.put("/api/settings", json=update_data)

    assert response.status_code == 200
    data = response.json()

    # Response should contain updated values
    assert data["jellyfin"]["server_url"] == "http://my-jellyfin:8096"
    assert data["jellyfin"]["api_key"] == "my-api-key"
    assert data["whisper"]["model"] == "small"

    # Verify YAML file was written
    from submate.config import load_yaml_config

    saved_config = load_yaml_config(config_path)
    assert saved_config["jellyfin"]["server_url"] == "http://my-jellyfin:8096"
    assert saved_config["jellyfin"]["api_key"] == "my-api-key"


def test_update_settings_partial(client: TestClient, config_path: Path, mocker):
    """Test PUT /api/settings with partial update only changes specified fields."""
    # Create existing config
    existing_config = {
        "jellyfin": {
            "server_url": "http://old-server:8096",
            "api_key": "old-key",
        },
        "whisper": {
            "model": "medium",
            "device": "cpu",
        },
    }
    save_yaml_config(config_path, existing_config)

    mocker.patch(
        "submate.server.handlers.settings.router._get_config_path",
        return_value=config_path,
    )

    # Update only whisper settings
    update_data = {
        "whisper": {
            "model": "large",
            "device": "cuda",
        }
    }

    response = client.put("/api/settings", json=update_data)

    assert response.status_code == 200
    data = response.json()

    # Whisper should be updated
    assert data["whisper"]["model"] == "large"
    assert data["whisper"]["device"] == "cuda"

    # Jellyfin should be preserved
    assert data["jellyfin"]["server_url"] == "http://old-server:8096"
    assert data["jellyfin"]["api_key"] == "old-key"


def test_test_jellyfin_success(client: TestClient, config_path: Path, mocker):
    """Test POST /api/settings/test-jellyfin returns success on valid config."""
    mocker.patch(
        "submate.server.handlers.settings.router._get_config_path",
        return_value=config_path,
    )

    # Mock the requests to Jellyfin
    mock_response = mocker.Mock()
    mock_response.status_code = 200
    mock_response.json.return_value = [
        {"Id": "lib-1", "Name": "Movies", "CollectionType": "movies"},
        {"Id": "lib-2", "Name": "TV Shows", "CollectionType": "tvshows"},
    ]
    mock_response.raise_for_status = mocker.Mock()

    mocker.patch("requests.get", return_value=mock_response)

    response = client.post(
        "/api/settings/test-jellyfin",
        json={
            "server_url": "http://jellyfin.local:8096",
            "api_key": "valid-api-key",
        },
    )

    assert response.status_code == 200
    data = response.json()

    assert data["success"] is True
    assert "Connected successfully" in data["message"]
    assert "libraries" in data["details"]
    assert len(data["details"]["libraries"]) == 2


def test_test_jellyfin_failure(client: TestClient, config_path: Path, mocker):
    """Test POST /api/settings/test-jellyfin returns failure on bad config."""
    mocker.patch(
        "submate.server.handlers.settings.router._get_config_path",
        return_value=config_path,
    )

    # Mock a failed request
    import requests

    mocker.patch(
        "requests.get",
        side_effect=requests.exceptions.ConnectionError("Connection refused"),
    )

    response = client.post(
        "/api/settings/test-jellyfin",
        json={
            "server_url": "http://invalid-server:8096",
            "api_key": "invalid-key",
        },
    )

    assert response.status_code == 200
    data = response.json()

    assert data["success"] is False
    assert "Connection" in data["message"] or "error" in data["message"].lower()


def test_test_jellyfin_missing_fields(client: TestClient, config_path: Path, mocker):
    """Test POST /api/settings/test-jellyfin returns failure when fields are missing."""
    mocker.patch(
        "submate.server.handlers.settings.router._get_config_path",
        return_value=config_path,
    )

    response = client.post(
        "/api/settings/test-jellyfin",
        json={
            "server_url": "",
            "api_key": "",
        },
    )

    assert response.status_code == 200
    data = response.json()

    assert data["success"] is False
    assert "required" in data["message"].lower() or "missing" in data["message"].lower()


def test_test_notification_success(client: TestClient, config_path: Path, mocker):
    """Test POST /api/settings/test-notification returns success."""
    mocker.patch(
        "submate.server.handlers.settings.router._get_config_path",
        return_value=config_path,
    )

    # Mock successful HTTP POST for webhook
    mock_response = mocker.Mock()
    mock_response.status_code = 200
    mock_response.raise_for_status = mocker.Mock()
    mocker.patch("requests.post", return_value=mock_response)

    response = client.post(
        "/api/settings/test-notification",
        json={
            "webhook_url": "https://hooks.example.com/webhook",
        },
    )

    assert response.status_code == 200
    data = response.json()

    assert data["success"] is True
    assert "sent" in data["message"].lower() or "success" in data["message"].lower()


def test_test_notification_ntfy(client: TestClient, config_path: Path, mocker):
    """Test POST /api/settings/test-notification with ntfy config."""
    mocker.patch(
        "submate.server.handlers.settings.router._get_config_path",
        return_value=config_path,
    )

    # Mock successful HTTP POST for ntfy
    mock_response = mocker.Mock()
    mock_response.status_code = 200
    mock_response.raise_for_status = mocker.Mock()
    mocker.patch("requests.post", return_value=mock_response)

    response = client.post(
        "/api/settings/test-notification",
        json={
            "ntfy_url": "https://ntfy.sh",
            "ntfy_topic": "submate-test",
        },
    )

    assert response.status_code == 200
    data = response.json()

    assert data["success"] is True


def test_test_notification_no_config(client: TestClient, config_path: Path, mocker):
    """Test POST /api/settings/test-notification returns error when no notification configured."""
    mocker.patch(
        "submate.server.handlers.settings.router._get_config_path",
        return_value=config_path,
    )

    response = client.post(
        "/api/settings/test-notification",
        json={
            "webhook_url": None,
            "ntfy_url": None,
            "ntfy_topic": None,
            "apprise_urls": [],
        },
    )

    assert response.status_code == 200
    data = response.json()

    assert data["success"] is False
    assert "no notification" in data["message"].lower() or "not configured" in data["message"].lower()


def test_test_notification_failure(client: TestClient, config_path: Path, mocker):
    """Test POST /api/settings/test-notification handles failures gracefully."""
    mocker.patch(
        "submate.server.handlers.settings.router._get_config_path",
        return_value=config_path,
    )

    # Mock a failed request
    import requests

    mocker.patch(
        "requests.post",
        side_effect=requests.exceptions.RequestException("Failed to send"),
    )

    response = client.post(
        "/api/settings/test-notification",
        json={
            "webhook_url": "https://hooks.example.com/invalid",
        },
    )

    assert response.status_code == 200
    data = response.json()

    assert data["success"] is False
    assert "fail" in data["message"].lower() or "error" in data["message"].lower()
