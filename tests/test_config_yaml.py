"""Tests for YAML configuration file utilities."""

import tempfile
from pathlib import Path


def test_load_yaml_config_basic():
    """Test loading basic YAML configuration."""
    from submate.config_yaml import load_yaml_config

    yaml_content = """
jellyfin:
  server_url: "http://jellyfin:8096"
  api_key: "test-key"
"""
    with tempfile.NamedTemporaryFile(mode="w", suffix=".yaml", delete=False) as f:
        f.write(yaml_content)
        f.flush()
        config = load_yaml_config(Path(f.name))

    assert config["jellyfin"]["server_url"] == "http://jellyfin:8096"
    assert config["jellyfin"]["api_key"] == "test-key"


def test_load_yaml_config_missing_file_returns_empty():
    """Test that missing file returns empty dict."""
    from submate.config_yaml import load_yaml_config

    config = load_yaml_config(Path("/nonexistent/config.yaml"))
    assert config == {}


def test_save_yaml_config():
    """Test saving configuration to YAML file."""
    from submate.config_yaml import load_yaml_config, save_yaml_config

    config = {
        "jellyfin": {"server_url": "http://localhost:8096", "api_key": "new-key"},
        "whisper": {"model": "large"},
    }

    with tempfile.NamedTemporaryFile(mode="w", suffix=".yaml", delete=False) as f:
        path = Path(f.name)

    save_yaml_config(path, config)
    loaded = load_yaml_config(path)

    assert loaded["jellyfin"]["server_url"] == "http://localhost:8096"
    assert loaded["whisper"]["model"] == "large"
