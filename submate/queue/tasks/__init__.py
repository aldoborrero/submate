from submate.queue.models import TaskResult
from submate.queue.tasks.base import BaseTask
from submate.queue.tasks.bazarr import BazarrTranscriptionTask, LanguageDetectionTask
from submate.queue.tasks.transcription import TranscriptionTask

__all__ = ["BaseTask", "TaskResult", "TranscriptionTask", "BazarrTranscriptionTask", "LanguageDetectionTask"]
