"""File-based transcription task."""

import logging
from pathlib import Path
from typing import Any

from submate.config import Config

from ..models import TaskResult, TranscriptionResult, TranscriptionSkippedError
from ..services import TranscriptionService
from .base import BaseTask

logger = logging.getLogger(__name__)


class TranscriptionTask(BaseTask[TranscriptionResult]):
    """File-based transcription task."""

    transcription_service: TranscriptionService

    @property
    def task_name(self) -> str:
        return "transcription"

    def __init__(self, config: Config, transcription_service: TranscriptionService, **kwargs: Any) -> None:
        super().__init__(config, transcription_service=transcription_service, **kwargs)
        self.transcription_service = transcription_service

    def validate_input(self, **kwargs: Any) -> None:
        file_path = kwargs.get("file_path")
        if file_path and not Path(file_path).exists():
            raise ValueError(f"File does not exist: {file_path}")

    def execute(self, **kwargs: Any) -> TaskResult[TranscriptionResult]:
        file_path = kwargs.get("file_path")
        if not file_path:
            return TaskResult(success=False, error="file_path is required")
        audio_language = kwargs.get("audio_language")
        translate_to = kwargs.get("translate_to")
        force = kwargs.get("force", False)
        try:
            result = self.transcription_service.transcribe_file(Path(file_path), audio_language, translate_to, force)
            return TaskResult(success=True, data=result)
        except TranscriptionSkippedError:
            # Let skip errors propagate for proper handling
            raise
        except Exception as e:
            logger.error("Transcription task failed for %s", file_path, exc_info=True)
            return TaskResult(success=False, error=str(e))
