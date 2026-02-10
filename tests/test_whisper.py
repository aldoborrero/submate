"""Tests for WhisperModelWrapper class."""

from unittest.mock import MagicMock, patch

import pytest

from submate.config import Config, StableTsSettings, WhisperSettings
from submate.types import Device, WhisperImplementation, WhisperModel
from submate.whisper import WhisperModelWrapper


@pytest.fixture
def config() -> Config:
    """Create test configuration."""
    return Config(
        whisper=WhisperSettings(
            model=WhisperModel.BASE,
            device=Device.CPU,
            compute_type="int8",
            implementation=WhisperImplementation.FASTER_WHISPER,
        ),
        stable_ts=StableTsSettings(custom_regroup="cm_sl=84_sl=42++++++1"),
    )


def test_whisper_model_init(config: Config) -> None:
    """Test WhisperModelWrapper can be instantiated."""
    model = WhisperModelWrapper(config)

    assert model is not None
    assert model.config == config
    assert not model.is_loaded


def test_whisper_model_invalid_model_size() -> None:
    """Test config validation rejects invalid model size."""
    from pydantic import ValidationError

    with pytest.raises(ValidationError, match="Invalid model"):
        Config(whisper=WhisperSettings(model="invalid", device="cpu"))


def test_whisper_model_invalid_device() -> None:
    """Test config validation rejects invalid device."""
    from pydantic import ValidationError

    with pytest.raises(ValidationError, match="Input should be"):
        Config(whisper=WhisperSettings(model="base", device="invalid"))


def test_load_model(config: Config) -> None:
    """Test model loading."""
    model = WhisperModelWrapper(config)

    with patch("stable_whisper.load_faster_whisper") as mock_load:
        mock_model = MagicMock()
        mock_load.return_value = mock_model

        model.load()

        assert model.is_loaded
        mock_load.assert_called_once_with(
            "base",
            device="cpu",
            compute_type="int8",
        )


def test_load_model_idempotent(config: Config) -> None:
    """Test that calling load() multiple times is safe."""
    model = WhisperModelWrapper(config)

    with patch("stable_whisper.load_faster_whisper") as mock_load:
        mock_model = MagicMock()
        mock_load.return_value = mock_model

        model.load()
        model.load()  # Second call should do nothing

        # Should only load once
        assert mock_load.call_count == 1


def test_load_model_failure(config: Config) -> None:
    """Test model loading failure raises RuntimeError."""
    model = WhisperModelWrapper(config)

    with patch("stable_whisper.load_faster_whisper") as mock_load:
        mock_load.side_effect = Exception("CUDA OOM")

        with pytest.raises(RuntimeError, match="Failed to load model"):
            model.load()

        assert not model.is_loaded


def test_unload_model(config: Config) -> None:
    """Test model unloading."""
    model = WhisperModelWrapper(config)

    with patch("stable_whisper.load_faster_whisper") as mock_load:
        mock_model = MagicMock()
        mock_load.return_value = mock_model

        model.load()
        assert model.is_loaded

        with patch("gc.collect") as mock_gc:
            model.unload()

            assert not model.is_loaded
            mock_gc.assert_called_once()


def test_unload_model_idempotent(config: Config) -> None:
    """Test that calling unload() multiple times is safe."""
    model = WhisperModelWrapper(config)

    with patch("stable_whisper.load_faster_whisper"):
        model.load()
        model.unload()
        model.unload()  # Should be safe

        assert not model.is_loaded


def test_unload_with_vram_cleanup(config: Config) -> None:
    """Test VRAM cleanup is called when configured."""
    # Enable VRAM cleanup
    config.clear_vram_on_complete = True

    model = WhisperModelWrapper(config)

    with patch("stable_whisper.load_faster_whisper") as mock_load:
        mock_model = MagicMock()
        mock_load.return_value = mock_model

        model.load()

        with patch("torch.cuda.is_available", return_value=True):
            with patch("torch.cuda.empty_cache") as mock_empty:
                with patch("torch.cuda.ipc_collect") as mock_ipc:
                    model.unload()

                    mock_empty.assert_called_once()
                    mock_ipc.assert_called_once()


def test_transcribe_with_path(config: Config) -> None:
    """Test transcribing from file path."""
    model = WhisperModelWrapper(config)

    with patch("stable_whisper.load_faster_whisper") as mock_load:
        mock_model = MagicMock()
        mock_result = MagicMock()
        mock_result.language = "en"
        mock_result.text = "Hello world"
        mock_model.transcribe_stable.return_value = mock_result
        mock_load.return_value = mock_model

        model.load()
        result = model.transcribe("/path/to/audio.wav", language="en")

        assert result.language == "en"
        assert result.text == "Hello world"

        mock_model.transcribe_stable.assert_called_once()
        call_args = mock_model.transcribe_stable.call_args
        assert call_args.args[0] == "/path/to/audio.wav"
        assert call_args.kwargs["language"] == "en"
        assert call_args.kwargs["task"] == "transcribe"


def test_transcribe_with_bytes(config: Config) -> None:
    """Test transcribing from bytes."""
    model = WhisperModelWrapper(config)

    with patch("stable_whisper.load_faster_whisper") as mock_load:
        mock_model = MagicMock()
        mock_result = MagicMock()
        mock_result.language = "es"
        mock_result.text = "Hola mundo"
        mock_model.transcribe_stable.return_value = mock_result
        mock_load.return_value = mock_model

        model.load()
        audio_bytes = b"fake_audio_data"
        result = model.transcribe(audio_bytes, language="es")

        assert result.language == "es"
        assert result.text == "Hola mundo"

        call_args = mock_model.transcribe_stable.call_args
        assert call_args.args[0] == audio_bytes


def test_transcribe_translate_task(config: Config) -> None:
    """Test transcribe with translate task."""
    model = WhisperModelWrapper(config)

    with patch("stable_whisper.load_faster_whisper") as mock_load:
        mock_model = MagicMock()
        mock_result = MagicMock()
        mock_model.transcribe_stable.return_value = mock_result
        mock_load.return_value = mock_model

        model.load()
        model.transcribe("/path/to/audio.wav", task="translate")

        call_args = mock_model.transcribe_stable.call_args
        assert call_args.kwargs["task"] == "translate"


def test_transcribe_invalid_task(config: Config) -> None:
    """Test invalid task raises ValueError."""
    model = WhisperModelWrapper(config)

    with patch("stable_whisper.load_faster_whisper"):
        model.load()

        with pytest.raises(ValueError, match="Invalid task"):
            model.transcribe("/path/to/audio.wav", task="invalid")  # type: ignore[arg-type]


def test_transcribe_model_not_loaded(config: Config) -> None:
    """Test transcribe without loading raises RuntimeError."""
    model = WhisperModelWrapper(config)

    with pytest.raises(RuntimeError, match="Model not loaded"):
        model.transcribe("/path/to/audio.wav")


def test_transcribe_failure(config: Config) -> None:
    """Test transcription failure raises RuntimeError."""
    model = WhisperModelWrapper(config)

    with patch("stable_whisper.load_faster_whisper") as mock_load:
        mock_model = MagicMock()
        mock_model.transcribe_stable.side_effect = Exception("Transcription error")
        mock_load.return_value = mock_model

        model.load()

        with pytest.raises(RuntimeError, match="Transcription failed"):
            model.transcribe("/path/to/audio.wav")


def test_context_manager_loads_and_unloads(config: Config) -> None:
    """Test context manager automatically loads and unloads."""
    with patch("stable_whisper.load_faster_whisper") as mock_load:
        mock_model = MagicMock()
        mock_load.return_value = mock_model

        model = WhisperModelWrapper(config)
        assert not model.is_loaded

        with model:
            assert model.is_loaded

        # After exiting context
        assert not model.is_loaded


def test_context_manager_cleanup_on_exception(config: Config) -> None:
    """Test context manager unloads even if exception occurs."""
    with patch("stable_whisper.load_faster_whisper") as mock_load:
        mock_model = MagicMock()
        mock_load.return_value = mock_model

        model = WhisperModelWrapper(config)

        with pytest.raises(ValueError):
            with model:
                assert model.is_loaded
                raise ValueError("Test error")

        # Model should still be unloaded despite exception
        assert not model.is_loaded


def test_context_manager_full_workflow(config: Config) -> None:
    """Test full workflow with context manager."""
    with patch("stable_whisper.load_faster_whisper") as mock_load:
        mock_model = MagicMock()
        mock_result = MagicMock()
        mock_result.language = "en"
        mock_result.text = "Test"
        mock_model.transcribe_stable.return_value = mock_result
        mock_load.return_value = mock_model

        with WhisperModelWrapper(config) as model:
            result = model.transcribe("/path/to/audio.wav")
            assert result.language == "en"
