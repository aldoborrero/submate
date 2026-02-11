# submate/queue/tasks/bazarr.py
import logging
from typing import Any

from submate.config import Config
from submate.queue.models import LanguageDetectionResult, OutputFormat, TaskResult
from submate.queue.services.bazarr import BazarrService

from .base import BaseTask

logger = logging.getLogger(__name__)


class BazarrTranscriptionTask(BaseTask[str]):
    """Bazarr transcription task."""

    bazarr_service: BazarrService

    def __init__(self, config: Config, bazarr_service: BazarrService, **kwargs: Any) -> None:
        super().__init__(config, bazarr_service=bazarr_service, **kwargs)
        self.bazarr_service = bazarr_service

    @property
    def task_name(self) -> str:
        return "bazarr_transcription"

    def execute(self, **kwargs: Any) -> TaskResult[str]:
        try:
            audio_bytes = kwargs["audio_bytes"]
            language = kwargs.get("language")
            output_format_input = kwargs.get("output_format", OutputFormat.SRT)
            word_timestamps = kwargs.get("word_timestamps", False)
            task = kwargs.get("task", "transcribe")
            target_language = kwargs.get("target_language")

            # Normalize output_format - handle both string and enum inputs
            if isinstance(output_format_input, str):
                try:
                    output_format = OutputFormat(output_format_input)
                except ValueError:
                    output_format = OutputFormat.SRT
            else:
                output_format = output_format_input

            subtitle_content = self.bazarr_service.transcribe_audio_bytes(
                audio_bytes=audio_bytes,
                language=language,
                task=task,
                output_format=output_format,
                word_timestamps=word_timestamps,
                target_language=target_language,
            )
            return TaskResult(success=True, data=subtitle_content)
        except Exception as e:
            logger.error("Bazarr ASR task failed", exc_info=True)
            return TaskResult(success=False, error=str(e))


class LanguageDetectionTask(BaseTask[LanguageDetectionResult]):
    """Language detection task for Bazarr."""

    bazarr_service: BazarrService

    def __init__(self, config: Config, bazarr_service: BazarrService, **kwargs: Any) -> None:
        super().__init__(config, bazarr_service=bazarr_service, **kwargs)
        self.bazarr_service = bazarr_service

    @property
    def task_name(self) -> str:
        return "language_detection"

    def execute(self, **kwargs: Any) -> TaskResult[LanguageDetectionResult]:
        try:
            audio_bytes = kwargs["audio_bytes"]
            result = self.bazarr_service.detect_language(audio_bytes)
            return TaskResult(success=True, data=result)
        except Exception as e:
            logger.error("Bazarr language detection task failed", exc_info=True)
            return TaskResult(
                success=False,
                error=str(e),
                data={
                    "detected_language": "Unknown",
                    "language_code": "und",
                },
            )
