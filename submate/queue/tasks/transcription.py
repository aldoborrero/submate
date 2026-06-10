"""File-based transcription task."""

from pathlib import Path
from typing import Any

from submate.config import Config

from ..models import TranscriptionResult, TranscriptionSkippedError
from ..services import TranscriptionService
from .base import BaseTask


class TranscriptionTask(BaseTask[TranscriptionResult]):
    """File-based transcription task."""

    transcription_service: TranscriptionService
    # Skips are an expected control-flow signal handled by the caller.
    propagate_exceptions = (TranscriptionSkippedError,)

    @property
    def task_name(self) -> str:
        return "transcription"

    def __init__(self, config: Config, transcription_service: TranscriptionService, **kwargs: Any) -> None:
        super().__init__(config, transcription_service=transcription_service, **kwargs)

    def validate_input(self, **kwargs: Any) -> None:
        file_path = kwargs.get("file_path")
        if file_path and not Path(file_path).exists():
            raise ValueError(f"File does not exist: {file_path}")

    def _run(self, **kwargs: Any) -> TranscriptionResult:
        file_path = kwargs.get("file_path")
        if not file_path:
            raise ValueError("file_path is required")
        audio_language = kwargs.get("audio_language")
        translate_to = kwargs.get("translate_to")
        force = kwargs.get("force", False)
        return self.transcription_service.transcribe_file(Path(file_path), audio_language, translate_to, force)
