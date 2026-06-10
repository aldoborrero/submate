# submate/queue/tasks/bazarr.py
from typing import Any

from submate.config import Config
from submate.queue.models import LanguageDetectionResult, OutputFormat
from submate.queue.services.bazarr import BazarrService

from .base import BaseTask


class BazarrTranscriptionTask(BaseTask[str]):
    """Bazarr transcription task."""

    bazarr_service: BazarrService

    def __init__(self, config: Config, bazarr_service: BazarrService, **kwargs: Any) -> None:
        super().__init__(config, bazarr_service=bazarr_service, **kwargs)

    @property
    def task_name(self) -> str:
        return "bazarr_transcription"

    def _run(self, **kwargs: Any) -> str:
        audio_bytes = kwargs["audio_bytes"]
        language = kwargs.get("language")
        word_timestamps = kwargs.get("word_timestamps", False)
        task = kwargs.get("task", "transcribe")
        target_language = kwargs.get("target_language")

        # Normalize output_format - handle both string and enum inputs
        output_format = OutputFormat.from_value(kwargs.get("output_format", OutputFormat.SRT))

        return self.bazarr_service.transcribe_audio_bytes(
            audio_bytes=audio_bytes,
            language=language,
            task=task,
            output_format=output_format,
            word_timestamps=word_timestamps,
            target_language=target_language,
        )


class LanguageDetectionTask(BaseTask[LanguageDetectionResult]):
    """Language detection task for Bazarr."""

    bazarr_service: BazarrService
    failure_data = {"detected_language": "Unknown", "language_code": "und"}

    def __init__(self, config: Config, bazarr_service: BazarrService, **kwargs: Any) -> None:
        super().__init__(config, bazarr_service=bazarr_service, **kwargs)

    @property
    def task_name(self) -> str:
        return "language_detection"

    def _run(self, **kwargs: Any) -> LanguageDetectionResult:
        audio_bytes = kwargs["audio_bytes"]
        return self.bazarr_service.detect_language(audio_bytes)
