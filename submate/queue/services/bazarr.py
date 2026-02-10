# submate/queue/services/bazarr.py
import json
import logging
from typing import Literal

from submate.config import Config
from submate.whisper import WhisperModelWrapper

from ..models import LanguageDetectionResult, OutputFormat

logger = logging.getLogger(__name__)


class BazarrService:
    """Handles Bazarr-specific operations."""

    # Language code to name mapping
    LANGUAGE_NAMES: dict[str, str] = {
        "en": "English",
        "es": "Spanish",
        "fr": "French",
        "de": "German",
        "it": "Italian",
        "pt": "Portuguese",
        "ru": "Russian",
        "ja": "Japanese",
        "zh": "Chinese",
        "ko": "Korean",
        "ar": "Arabic",
        "hi": "Hindi",
        "nl": "Dutch",
        "pl": "Polish",
        "tr": "Turkish",
        "vi": "Vietnamese",
        "th": "Thai",
        "sv": "Swedish",
        "da": "Danish",
        "fi": "Finnish",
        "no": "Norwegian",
        "cs": "Czech",
        "el": "Greek",
        "he": "Hebrew",
        "hu": "Hungarian",
        "id": "Indonesian",
        "ms": "Malay",
        "ro": "Romanian",
        "sk": "Slovak",
        "uk": "Ukrainian",
    }

    def __init__(self, config: Config):
        self.config = config

    def detect_language(self, audio_bytes: bytes) -> LanguageDetectionResult:
        """Detect language from audio bytes.

        Args:
            audio_bytes: Raw audio data

        Returns:
            LanguageDetectionResult with detected language info
        """
        logger.info("Detecting language from audio (%d bytes)", len(audio_bytes))
        try:
            with WhisperModelWrapper(self.config) as model:
                # Transcribe a short segment to detect language
                result = model.transcribe(audio_bytes)
                language_code = result.language or "und"
                language_name = self.LANGUAGE_NAMES.get(language_code, "Unknown")

                logger.info("Language detected: %s (%s)", language_name, language_code)
                return {
                    "detected_language": language_name,
                    "language_code": language_code,
                }
        except Exception:
            logger.error("Failed to detect language", exc_info=True)
            raise

    def transcribe_audio_bytes(
        self,
        audio_bytes: bytes,
        language: str | None = None,
        task: Literal["transcribe", "translate"] = "transcribe",
        output_format: OutputFormat = OutputFormat.SRT,
        word_timestamps: bool = False,
    ) -> str:
        """Transcribe audio bytes for Bazarr."""
        logger.info(
            "Transcribing audio (%d bytes): task=%s, language=%s, format=%s",
            len(audio_bytes),
            task,
            language,
            output_format.value,
        )
        try:
            with WhisperModelWrapper(self.config) as model:
                result = model.transcribe(audio_bytes, language=language, task=task)

                # Return formatted output based on enum
                match output_format:
                    case OutputFormat.SRT:
                        content = result.to_srt_vtt(filepath=None, word_level=word_timestamps)
                        return content if content is not None else ""
                    case OutputFormat.VTT:
                        content = result.to_srt_vtt(filepath=None, word_level=word_timestamps, vtt=True)
                        return content if content is not None else ""
                    case OutputFormat.TXT:
                        return result.text
                    case OutputFormat.JSON:
                        return json.dumps(result.to_dict())
                    case _:
                        raise ValueError(f"Unsupported output format: {output_format}")
        except Exception:
            logger.error(
                "Failed to transcribe audio (task=%s, format=%s)",
                task,
                output_format.value,
                exc_info=True,
            )
            raise
