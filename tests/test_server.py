"""Tests for FastAPI server and Bazarr endpoints."""

from io import BytesIO

import pytest
from fastapi.testclient import TestClient

from submate.server import app
from submate.server.handlers.bazarr.handlers import handle_asr_request
from submate.server.handlers.bazarr.models import LanguageDetectionResponse


@pytest.fixture
def client():
    """FastAPI test client."""
    return TestClient(app)


# Core server endpoints


def test_root_endpoint(client):
    """Test root endpoint."""
    response = client.get("/")

    assert response.status_code == 200
    data = response.json()
    assert "name" in data
    assert "version" in data


def test_status_endpoint(client, mocker):
    """Test /status endpoint."""
    mock_task_queue = mocker.MagicMock()
    type(mock_task_queue).stats = mocker.PropertyMock(return_value={"pending": 0, "scheduled": 0})
    mocker.patch("submate.server.handlers.core.router.get_task_queue", return_value=mock_task_queue)

    response = client.get("/status")

    assert response.status_code == 200
    data = response.json()
    assert data["status"] == "ok"


def test_queue_endpoint(client, mocker):
    """Test /queue endpoint."""
    mock_task_queue = mocker.MagicMock()
    type(mock_task_queue).stats = mocker.PropertyMock(return_value={"pending": 5, "scheduled": 2})
    mocker.patch("submate.server.handlers.core.router.get_task_queue", return_value=mock_task_queue)

    response = client.get("/queue")

    assert response.status_code == 200
    data = response.json()
    assert data["pending"] == 5
    assert data["scheduled"] == 2


# Bazarr endpoints


def test_bazarr_asr_endpoint(client, mocker):
    """Test /bazarr/asr endpoint."""
    mock_result = mocker.MagicMock()
    mock_result.return_value = {"success": True, "data": "1\n00:00:00,000 --> 00:00:05,000\nTest subtitle\n"}
    mock_task = mocker.MagicMock(return_value=mock_result)
    mocker.patch("submate.server.handlers.bazarr.handlers.transcribe_audio_task", mock_task)

    response = client.post(
        "/bazarr/asr",
        params={"task": "transcribe", "language": "en", "output": "srt"},
        files={"audio_file": ("test.wav", BytesIO(b"fake_audio"), "audio/wav")},
    )

    assert response.status_code == 200
    assert "Test subtitle" in response.text


def test_bazarr_detect_language_endpoint(client, mocker):
    """Test /bazarr/detect-language endpoint."""
    mocker.patch(
        "submate.server.handlers.bazarr.handlers.extract_audio_segment",
        return_value=b"fake_segment",
    )
    mock_result = mocker.MagicMock()
    mock_result.return_value = {"success": True, "data": {"detected_language": "English", "language_code": "en"}}
    mock_task = mocker.MagicMock(return_value=mock_result)
    mocker.patch("submate.server.handlers.bazarr.handlers.detect_language_task", mock_task)

    response = client.post(
        "/bazarr/detect-language",
        params={"detect_lang_length": 30},
        files={"audio_file": ("test.wav", BytesIO(b"fake_audio"), "audio/wav")},
    )

    assert response.status_code == 200
    assert response.json()["language_code"] == "en"


# Bazarr handler unit tests


@pytest.mark.asyncio
async def test_handle_asr_transcribe(mocker):
    """Test ASR transcription handler."""
    mock_result = mocker.MagicMock()
    mock_result.return_value = {"success": True, "data": "1\n00:00:00,000 --> 00:00:05,000\nTest\n"}
    mock_task = mocker.MagicMock(return_value=mock_result)
    mocker.patch("submate.server.handlers.bazarr.handlers.transcribe_audio_task", mock_task)

    result = await handle_asr_request(
        audio_file=BytesIO(b"audio"),
        task="transcribe",
        language="en",
        output="srt",
        encode=True,
    )

    assert "Test" in result
    mock_task.assert_called_once()


@pytest.mark.asyncio
async def test_handle_asr_translate(mocker):
    """Test ASR translation handler."""
    mock_result = mocker.MagicMock()
    mock_result.return_value = {"success": True, "data": "1\n00:00:00,000 --> 00:00:05,000\nTranslated\n"}
    mock_task = mocker.MagicMock(return_value=mock_result)
    mocker.patch("submate.server.handlers.bazarr.handlers.transcribe_audio_task", mock_task)

    result = await handle_asr_request(
        audio_file=BytesIO(b"audio"),
        task="translate",
        language=None,
        output="srt",
        encode=True,
    )

    assert "Translated" in result


@pytest.mark.asyncio
async def test_handle_asr_encode_false_accepted(mocker):
    """Test ASR accepts encode=False (Bazarr compatibility)."""
    mock_result = mocker.MagicMock()
    mock_result.return_value = {"success": True, "data": "1\n00:00:00,000 --> 00:00:05,000\nTest\n"}
    mock_task = mocker.MagicMock(return_value=mock_result)
    mocker.patch("submate.server.handlers.bazarr.handlers.transcribe_audio_task", mock_task)

    result = await handle_asr_request(
        audio_file=BytesIO(b"audio"),
        task="transcribe",
        language="en",
        output="srt",
        encode=False,
    )

    assert result is not None


# Bazarr model tests


def test_language_detection_response_model():
    """Test LanguageDetectionResponse model."""
    response = LanguageDetectionResponse(detected_language="English", language_code="en")
    assert response.detected_language == "English"
    assert response.language_code == "en"


def test_language_detection_response_from_dict():
    """Test LanguageDetectionResponse from dict."""
    response = LanguageDetectionResponse.model_validate({"detected_language": "Spanish", "language_code": "es"})
    assert response.language_code == "es"
