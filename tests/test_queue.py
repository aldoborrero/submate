"""Tests for queue tasks - transcription and Bazarr."""

from pathlib import Path
from unittest.mock import Mock, patch

import pytest

from submate.queue.models import OutputFormat, TranscriptionResult
from submate.queue.tasks.bazarr import BazarrTranscriptionTask, LanguageDetectionTask
from submate.queue.tasks.transcription import TranscriptionTask

# Transcription task tests


def test_transcription_validate_input_valid():
    """Test validation passes for existing file."""
    task = TranscriptionTask(Mock(), transcription_service=Mock())

    with patch("pathlib.Path.exists", return_value=True):
        task.validate_input(file_path="/valid.mp4")


def test_transcription_validate_input_invalid():
    """Test validation fails for non-existent file."""
    task = TranscriptionTask(Mock(), transcription_service=Mock())

    with pytest.raises(ValueError, match="File does not exist"):
        task.validate_input(file_path="/nonexistent.mp4")


def test_transcription_execute_success():
    """Test successful transcription execution."""
    service = Mock()
    task = TranscriptionTask(Mock(), transcription_service=service)

    mock_result = TranscriptionResult(subtitle_path="/path/to/sub.srt", language="en", segments=5, text="Test")
    service.transcribe_file.return_value = mock_result

    result = task.execute(file_path="/input.mp4", audio_language="en", translate_to=None, force=False)

    assert result.success is True
    assert result.data == mock_result
    service.transcribe_file.assert_called_once_with(Path("/input.mp4"), "en", None, False)


def test_transcription_execute_failure():
    """Test transcription failure returns error result."""
    service = Mock()
    task = TranscriptionTask(Mock(), transcription_service=service)

    service.transcribe_file.side_effect = Exception("Transcription failed")

    result = task.execute(file_path="/input.mp4")

    assert result.success is False
    assert result.error == "Transcription failed"


# Bazarr task tests


def test_bazarr_transcription_execute():
    """Test Bazarr transcription task execution."""
    service = Mock()
    task = BazarrTranscriptionTask(Mock(), bazarr_service=service)

    service.transcribe_audio_bytes.return_value = "subtitle content"

    result = task.execute(
        audio_bytes=b"test_audio",
        language="en",
        task="transcribe",
        output_format=OutputFormat.SRT,
        word_timestamps=True,
        target_language=None,
    )

    assert result.success is True
    assert result.data == "subtitle content"
    service.transcribe_audio_bytes.assert_called_once_with(
        audio_bytes=b"test_audio",
        language="en",
        task="transcribe",
        output_format=OutputFormat.SRT,
        word_timestamps=True,
        target_language=None,
    )


def test_bazarr_transcription_with_translation():
    """Test Bazarr transcription with target language for translation."""
    service = Mock()
    task = BazarrTranscriptionTask(Mock(), bazarr_service=service)

    # Service should return translated content
    service.transcribe_audio_bytes.return_value = "contenido de subtítulos traducido"

    result = task.execute(
        audio_bytes=b"test_audio",
        language=None,  # Auto-detect source
        task="transcribe",
        output_format=OutputFormat.SRT,
        word_timestamps=False,
        target_language="es",  # Translate to Spanish
    )

    assert result.success is True
    assert result.data == "contenido de subtítulos traducido"
    service.transcribe_audio_bytes.assert_called_once_with(
        audio_bytes=b"test_audio",
        language=None,
        task="transcribe",
        output_format=OutputFormat.SRT,
        word_timestamps=False,
        target_language="es",
    )


def test_language_detection_execute():
    """Test language detection task execution."""
    service = Mock()
    task = LanguageDetectionTask(Mock(), bazarr_service=service)

    expected = {"detected_language": "English", "language_code": "en"}
    service.detect_language.return_value = expected

    result = task.execute(audio_bytes=b"test_audio")

    assert result.success is True
    assert result.data == expected
    service.detect_language.assert_called_once_with(b"test_audio")
