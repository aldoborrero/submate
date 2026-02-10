from dataclasses import dataclass
from enum import Enum, StrEnum
from typing import Any, TypedDict, TypeVar

T = TypeVar("T")


class OutputFormat(Enum):
    """Supported output formats for transcription."""

    SRT = "srt"
    VTT = "vtt"
    TXT = "txt"
    JSON = "json"

    @property
    def extension(self) -> str:
        return f".{self.value}"


class SkipReason(StrEnum):
    """Reasons for skipping transcription."""

    # Not skipped
    NOT_SKIPPED = "not_skipped"

    # Subtitle existence checks
    TARGET_SUBTITLE_EXISTS = "target_subtitle_exists"
    EXTERNAL_SUBTITLE_EXISTS = "external_subtitle_exists"
    INTERNAL_SUBTITLE_LANGUAGE_EXISTS = "internal_subtitle_language_exists"

    # Language-based skipping
    SUBTITLE_LANGUAGE_IN_SKIP_LIST = "subtitle_language_in_skip_list"
    AUDIO_LANGUAGE_IN_SKIP_LIST = "audio_language_in_skip_list"
    UNKNOWN_LANGUAGE = "unknown_language"

    # Preference-based skipping
    NO_PREFERRED_AUDIO_LANGUAGE = "no_preferred_audio_language"

    # Audio file specific
    LRC_FILE_EXISTS = "lrc_file_exists"

    # Special cases
    LANGUAGE_NOT_SET_BUT_SUBTITLES_EXIST = "language_not_set_but_subtitles_exist"


@dataclass
class TaskResult[T]:
    """Unified result wrapper for all tasks."""

    success: bool
    data: T | None = None
    error: str | None = None
    metadata: dict[str, Any] | None = None


@dataclass
class TranscriptionResult:
    """Result of a transcription operation."""

    subtitle_path: str
    language: str
    segments: int
    text: str


class LanguageDetectionResult(TypedDict):
    """Result of language detection."""

    detected_language: str
    language_code: str


class TranscriptionSkippedError(Exception):
    """Raised when transcription should be skipped."""

    def __init__(self, reason: SkipReason, message: str | None = None):
        self.reason = reason
        self.message = message or reason.value
        super().__init__(self.message)
