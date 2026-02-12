"""Tests for YAML configuration file utilities and Pydantic integration."""

import tempfile
from pathlib import Path


def test_load_yaml_config_basic():
    """Test loading basic YAML configuration."""
    from submate.config import load_yaml_config

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
    from submate.config import load_yaml_config

    config = load_yaml_config(Path("/nonexistent/config.yaml"))
    assert config == {}


def test_save_yaml_config():
    """Test saving configuration to YAML file."""
    from submate.config import load_yaml_config, save_yaml_config

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


# ============================================================================
# Pydantic Integration Tests
# ============================================================================


def test_yaml_settings_source_loads_into_config():
    """Test that YAML values are loaded into the Pydantic Config object."""
    from submate.config import get_config

    yaml_content = """
whisper:
  model: "large"
  device: "cuda"
server:
  port: 8080
jellyfin:
  server_url: "http://jellyfin:8096"
  api_key: "yaml-api-key"
debug: true
"""
    with tempfile.NamedTemporaryFile(mode="w", suffix=".yaml", delete=False) as f:
        f.write(yaml_content)
        f.flush()
        yaml_path = Path(f.name)

    config = get_config(config_file=yaml_path)

    assert config.whisper.model == "large"
    assert config.whisper.device == "cuda"
    assert config.server.port == 8080
    assert config.jellyfin.server_url == "http://jellyfin:8096"
    assert config.jellyfin.api_key == "yaml-api-key"
    assert config.debug is True


def test_env_vars_override_yaml_values(monkeypatch):
    """Test that environment variables override YAML configuration."""
    from submate.config import get_config

    yaml_content = """
whisper:
  model: "small"
server:
  port: 7000
debug: false
"""
    with tempfile.NamedTemporaryFile(mode="w", suffix=".yaml", delete=False) as f:
        f.write(yaml_content)
        f.flush()
        yaml_path = Path(f.name)

    # Set environment variables that should override YAML
    monkeypatch.setenv("SUBMATE__WHISPER__MODEL", "large")
    monkeypatch.setenv("SUBMATE__SERVER__PORT", "9999")
    monkeypatch.setenv("SUBMATE__DEBUG", "true")

    config = get_config(config_file=yaml_path)

    # Env vars should override YAML values
    assert config.whisper.model == "large"
    assert config.server.port == 9999
    assert config.debug is True


def test_yaml_partial_config_uses_defaults():
    """Test that partial YAML config falls back to defaults for missing fields."""
    from submate.config import get_config

    yaml_content = """
whisper:
  model: "tiny"
"""
    with tempfile.NamedTemporaryFile(mode="w", suffix=".yaml", delete=False) as f:
        f.write(yaml_content)
        f.flush()
        yaml_path = Path(f.name)

    config = get_config(config_file=yaml_path)

    # YAML value should be used
    assert config.whisper.model == "tiny"
    # Defaults should be used for missing fields
    assert config.whisper.device == "cpu"  # default
    assert config.server.port == 9000  # default
    assert config.debug is False  # default


def test_yaml_with_nested_settings(monkeypatch):
    """Test YAML with deeply nested configuration."""
    from submate.config import get_config

    # Clear any env vars that might interfere
    monkeypatch.delenv("SUBMATE__TRANSLATION__BACKEND", raising=False)
    monkeypatch.delenv("SUBMATE__TRANSLATION__ANTHROPIC_API_KEY", raising=False)

    yaml_content = """
whisper:
  model: "medium"
  device: "cpu"
  implementation: "faster-whisper"
  compute_type: "float16"
stable_ts:
  word_level_highlight: true
  suppress_silence: false
  min_word_duration: 0.2
translation:
  backend: "ollama"
  ollama_model: "llama3.2"
  chunk_size: 100
"""
    with tempfile.NamedTemporaryFile(mode="w", suffix=".yaml", delete=False) as f:
        f.write(yaml_content)
        f.flush()
        yaml_path = Path(f.name)

    config = get_config(config_file=yaml_path)

    assert config.whisper.model == "medium"
    assert config.whisper.implementation == "faster-whisper"
    assert config.whisper.compute_type == "float16"
    assert config.stable_ts.word_level_highlight is True
    assert config.stable_ts.suppress_silence is False
    assert config.stable_ts.min_word_duration == 0.2
    # Use .value for enum comparison
    assert config.translation.backend.value == "ollama"
    assert config.translation.ollama_model == "llama3.2"
    assert config.translation.chunk_size == 100


def test_yaml_with_list_fields():
    """Test YAML with list fields (folders, libraries)."""
    from submate.config import get_config

    yaml_content = """
whisper:
  folders:
    - "/media/movies"
    - "/media/tv"
    - "/media/music"
jellyfin:
  libraries:
    - "Movies"
    - "TV Shows"
"""
    with tempfile.NamedTemporaryFile(mode="w", suffix=".yaml", delete=False) as f:
        f.write(yaml_content)
        f.flush()
        yaml_path = Path(f.name)

    config = get_config(config_file=yaml_path)

    assert config.whisper.folders == ["/media/movies", "/media/tv", "/media/music"]
    assert config.jellyfin.libraries == ["Movies", "TV Shows"]


def test_yaml_settings_source_with_nonexistent_file():
    """Test that nonexistent YAML file raises FileNotFoundError."""
    import pytest

    from submate.config import get_config

    with pytest.raises(FileNotFoundError):
        get_config(config_file=Path("/nonexistent/config.yaml"))


def test_yaml_string_path_accepted():
    """Test that get_config accepts string paths for YAML."""
    from submate.config import get_config

    yaml_content = """
debug: true
"""
    with tempfile.NamedTemporaryFile(mode="w", suffix=".yaml", delete=False) as f:
        f.write(yaml_content)
        f.flush()
        yaml_path_str = f.name  # String path

    config = get_config(config_file=yaml_path_str)

    assert config.debug is True


def test_combined_env_file_and_yaml(monkeypatch, tmp_path):
    """Test using env vars and YAML file together."""
    from submate.config import get_config

    # Set env var (env vars override YAML)
    monkeypatch.setenv("SUBMATE__SERVER__PORT", "7777")

    # Create YAML file
    yaml_content = """
whisper:
  model: "tiny"
debug: true
"""
    yaml_file = tmp_path / "config.yaml"
    yaml_file.write_text(yaml_content)

    config = get_config(config_file=yaml_file)

    # Env var should override
    assert config.server.port == 7777
    # YAML values
    assert config.whisper.model == "tiny"
    assert config.debug is True
