"""Tests for transcription skip-condition logic."""

from pathlib import Path

from submate.config import Config
from submate.language import LanguageCode
from submate.queue.models import SkipReason
from submate.queue.services.transcription import TranscriptionService


def _service(**subtitle_overrides) -> TranscriptionService:
    config = Config()
    for key, value in subtitle_overrides.items():
        setattr(config.subtitle, key, value)
    return TranscriptionService(config)


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
