"""Tests for configuration management."""

import pytest
from pydantic import ValidationError

from submate.config import Config, get_config


def test_config_defaults():
    """Test that Config has sensible defaults."""
    config = Config()
    assert config.whisper.model == "medium"
    assert config.server.port == 9000
    assert config.whisper.device == "cpu"
    assert config.debug is False
    assert config.clear_vram_on_complete is False


def test_config_from_env(monkeypatch):
    """Test loading configuration from environment variables."""
    monkeypatch.setenv("SUBMATE__WHISPER__MODEL", "large")
    monkeypatch.setenv("SUBMATE__SERVER__PORT", "8080")
    monkeypatch.setenv("SUBMATE__DEBUG", "true")

    config = get_config()
    assert config.whisper.model == "large"
    assert config.server.port == 8080
    assert config.debug is True


def test_config_bool_conversion(monkeypatch):
    """Test various boolean conversion formats."""
    test_cases = [
        ("true", True),
        ("True", True),
        ("1", True),
        ("yes", True),
        ("on", True),
        ("false", False),
        ("False", False),
        ("0", False),
        ("no", False),
        ("off", False),
    ]

    for value, expected in test_cases:
        monkeypatch.setenv("SUBMATE__DEBUG", value)
        config = get_config()
        assert config.debug is expected, f"Failed for value: {value}"


def test_config_use_path_mapping(monkeypatch):
    """Test path mapping configuration."""
    monkeypatch.setenv("SUBMATE__PATH_MAPPING__ENABLED", "true")
    monkeypatch.setenv("SUBMATE__PATH_MAPPING__FROM_PATH", "/host/media")
    monkeypatch.setenv("SUBMATE__PATH_MAPPING__TO_PATH", "/container/media")

    config = get_config()
    assert config.path_mapping.enabled is True
    assert config.path_mapping.from_path == "/host/media"
    assert config.path_mapping.to_path == "/container/media"


def test_config_no_emby_fields():
    """Test that Emby fields have been removed from Config."""
    config = Config()
    assert not hasattr(config, "emby_libraries")
    assert not hasattr(config, "emby_api_key")
    assert not hasattr(config, "emby_server_url")


def test_stable_ts_config(monkeypatch):
    """Test stable-ts configuration options."""
    monkeypatch.setenv("SUBMATE__STABLE_TS__WORD_LEVEL_HIGHLIGHT", "true")
    monkeypatch.setenv("SUBMATE__STABLE_TS__CUSTOM_REGROUP", "custom_pattern")
    monkeypatch.setenv("SUBMATE__STABLE_TS__SUPPRESS_SILENCE", "false")
    monkeypatch.setenv("SUBMATE__STABLE_TS__MIN_WORD_DURATION", "0.2")

    config = get_config()

    assert config.stable_ts.word_level_highlight is True
    assert config.stable_ts.custom_regroup == "custom_pattern"
    assert config.stable_ts.suppress_silence is False
    assert config.stable_ts.min_word_duration == 0.2


def test_pydantic_config_validation(monkeypatch):
    """Test Pydantic validation for config fields."""
    from pydantic import ValidationError

    # Test invalid type
    monkeypatch.setenv("SUBMATE__SERVER__PORT", "not_a_number")

    with pytest.raises(ValidationError):
        get_config()


def test_config_from_dict():
    """Test creating config from dictionary."""
    config = Config(
        whisper={"model": "large", "device": "cuda"},
        server={"port": 8000},
    )

    assert config.whisper.model == "large"
    assert config.whisper.device == "cuda"
    assert config.server.port == 8000


def test_config_validation_invalid_bool(monkeypatch):
    """Test that invalid bool raises ValidationError."""
    from pydantic import ValidationError

    monkeypatch.setenv("SUBMATE__DEBUG", "maybe")

    # Pydantic raises ValidationError for invalid bool strings
    with pytest.raises(ValidationError):
        get_config()


def test_config_parse_folders(monkeypatch):
    """Test parsing pipe-separated folders."""
    monkeypatch.setenv("SUBMATE__WHISPER__FOLDERS", "/media/movies|/media/tv|/media/music")

    config = get_config()

    assert config.whisper.folders == ["/media/movies", "/media/tv", "/media/music"]


def test_config_parse_regroup_false(monkeypatch):
    """Test parsing regroup=false."""
    monkeypatch.setenv("SUBMATE__STABLE_TS__CUSTOM_REGROUP", "false")

    config = get_config()

    assert config.stable_ts.custom_regroup is False


def test_config_parse_regroup_pattern(monkeypatch):
    """Test parsing regroup pattern."""
    monkeypatch.setenv("SUBMATE__STABLE_TS__CUSTOM_REGROUP", "cm_sl=100")

    config = get_config()

    assert config.stable_ts.custom_regroup == "cm_sl=100"


def test_config_xdg_queue_path():
    """Test default queue path uses XDG."""
    config = Config()

    assert "subgen" in config.queue.db_path
    assert "queue.db" in config.queue.db_path


def test_config_parse_jellyfin_libraries(monkeypatch):
    """Test parsing pipe-separated Jellyfin libraries."""
    monkeypatch.setenv("SUBMATE__JELLYFIN__LIBRARIES", "Movies|TV Shows|Music")

    config = get_config()

    assert config.jellyfin.libraries == ["Movies", "TV Shows", "Music"]


def test_server_webhook_triggers(monkeypatch):
    """Test webhook trigger configuration."""
    monkeypatch.setenv("SUBMATE__SERVER__PROCESS_ON_ADD", "true")
    monkeypatch.setenv("SUBMATE__SERVER__PROCESS_ON_PLAY", "false")

    config = get_config()

    assert config.server.process_on_add is True
    assert config.server.process_on_play is False


def test_bazarr_config(monkeypatch):
    """Test Bazarr configuration."""
    monkeypatch.setenv("SUBMATE__SERVER__BAZARR_KEEP_MODEL_LOADED", "true")
    monkeypatch.setenv("SUBMATE__SERVER__BAZARR_MODEL_IDLE_TIMEOUT", "600")

    config = get_config()

    assert config.server.bazarr_keep_model_loaded is True
    assert config.server.bazarr_model_idle_timeout == 600


def test_whisper_config_invalid_implementation(monkeypatch):
    """Test invalid implementation raises ValidationError."""
    monkeypatch.setenv("SUBMATE__WHISPER__IMPLEMENTATION", "invalid")
    with pytest.raises(ValidationError):
        get_config()


def test_whisper_config_invalid_device(monkeypatch):
    """Test invalid device raises ValidationError."""
    monkeypatch.setenv("SUBMATE__WHISPER__DEVICE", "invalid")
    with pytest.raises(ValidationError):
        get_config()


def test_whisper_config_invalid_model_for_implementation(monkeypatch):
    """Test invalid model for implementation raises ValidationError."""
    monkeypatch.setenv("SUBMATE__WHISPER__IMPLEMENTATION", "faster-whisper")
    monkeypatch.setenv("SUBMATE__WHISPER__MODEL", "invalid-model")
    with pytest.raises(ValidationError):
        get_config()


def test_whisper_config_valid_hf_model(monkeypatch):
    """Test valid HF model format."""
    monkeypatch.setenv("SUBMATE__WHISPER__IMPLEMENTATION", "hf-whisper")
    monkeypatch.setenv("SUBMATE__WHISPER__MODEL", "openai/whisper-tiny")
    config = get_config()
    assert config.whisper.implementation == "hf-whisper"
    assert config.whisper.model == "openai/whisper-tiny"
