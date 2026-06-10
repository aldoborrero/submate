"""Tests for transcription skip-condition logic."""

from pathlib import Path

import pytest

from submate.config import Config
from submate.language import LanguageCode
from submate.queue.models import SkipReason
from submate.queue.services.transcription import TranscriptionService


def _service(**subtitle_overrides) -> TranscriptionService:
    config = Config()
    for key, value in subtitle_overrides.items():
        setattr(config.subtitle, key, value)
    return TranscriptionService(config)


def _mock_transcription(service, subtitle_path, mocker, *, srt_body="1\n00:00:00,000 --> 00:00:01,000\nhi\n"):
    """Wire up a service so transcribe_file runs without real Whisper/media I/O.

    The mocked model's to_srt_vtt writes ``srt_body`` to whatever path it is
    given (the temp file), so the test can assert the atomic publish behavior.
    """
    mocker.patch.object(service, "_should_skip_transcription", return_value=(False, SkipReason.NOT_SKIPPED))

    result = mocker.MagicMock()
    result.language = "en"
    result.text = "hi"
    result.segments = [1]
    result.to_srt_vtt.side_effect = lambda path, word_level=False: Path(path).write_text(srt_body, encoding="utf-8")

    model = mocker.MagicMock()
    model.transcribe.return_value = result

    mocker.patch("submate.queue.services.transcription.get_whisper_model", return_value=model)
    mocker.patch("submate.queue.services.transcription.prepare_audio_for_transcription", return_value="audio")
    mocker.patch("submate.queue.services.transcription.build_subtitle_path", return_value=str(subtitle_path))
    return result


def test_transcribe_file_publishes_atomically_without_temp_leftover(tmp_path, mocker):
    """The finished subtitle is moved into place and no .tmp file is left behind."""
    service = _service()
    subtitle_path = tmp_path / "movie.en.srt"
    srt_body = "1\n00:00:00,000 --> 00:00:01,000\nhi\n"
    _mock_transcription(service, subtitle_path, mocker, srt_body=srt_body)

    out = service.transcribe_file(Path("/movie.mkv"), audio_language=None, translate_to=None, force=True)

    assert out.subtitle_path == str(subtitle_path)
    assert subtitle_path.read_text(encoding="utf-8") == srt_body
    assert list(tmp_path.glob("*.tmp")) == []


def test_transcribe_file_leaves_no_temp_on_failure(tmp_path, mocker):
    """A failure mid-render cleans up the temp file and never publishes a partial."""
    service = _service()
    subtitle_path = tmp_path / "movie.en.srt"
    result = _mock_transcription(service, subtitle_path, mocker)
    result.to_srt_vtt.side_effect = RuntimeError("boom")

    with pytest.raises(RuntimeError, match="boom"):
        service.transcribe_file(Path("/movie.mkv"), audio_language=None, translate_to=None, force=True)

    assert not subtitle_path.exists()
    assert list(tmp_path.glob("*.tmp")) == []


def test_skip_unknown_language_triggers_for_none_sentinel():
    """LanguageCode.from_string returns the NONE sentinel (never Python None),
    so the unknown-language skip must key off falsiness, not `is None`."""
    service = _service(skip_unknown_language=True)

    skip, reason = service._should_skip_transcription(Path("/movie.mkv"), LanguageCode.NONE)

    assert skip is True
    assert reason == SkipReason.UNKNOWN_LANGUAGE


def test_skip_unknown_language_not_triggered_for_real_language():
    service = _service(skip_unknown_language=True, skip_if_target_subtitle_exists=False)

    skip, reason = service._should_skip_transcription(Path("/movie.mkv"), LanguageCode.ENGLISH)

    assert reason != SkipReason.UNKNOWN_LANGUAGE
