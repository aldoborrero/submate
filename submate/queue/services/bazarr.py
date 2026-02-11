# submate/queue/services/bazarr.py
import json
import logging
from typing import Literal

from submate.config import Config
from submate.translation import TranslationService
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
        target_language: str | None = None,
    ) -> str:
        """Transcribe audio bytes for Bazarr.

        Args:
            audio_bytes: Raw audio data
            language: Source language hint for Whisper (None for auto-detect)
            task: "transcribe" or "translate" (Whisper's translate is to English only)
            output_format: Output format (srt, vtt, txt, json)
            word_timestamps: Enable word-level timestamps
            target_language: Target language for subtitles. If different from
                transcribed language, translation will be performed.

        Returns:
            Subtitle content as string, optionally translated to target_language
        """
        logger.info(
            "Transcribing audio (%d bytes): task=%s, language=%s, target=%s, format=%s",
            len(audio_bytes),
            task,
            language,
            target_language,
            output_format.value,
        )
        try:
            with WhisperModelWrapper(self.config) as model:
                result = model.transcribe(audio_bytes, language=language, task=task)
                detected_language = result.language or "en"

                # Get formatted output based on enum
                match output_format:
                    case OutputFormat.SRT:
                        content = result.to_srt_vtt(filepath=None, word_level=word_timestamps)
                        content = content if content is not None else ""
                    case OutputFormat.VTT:
                        content = result.to_srt_vtt(filepath=None, word_level=word_timestamps, vtt=True)
                        content = content if content is not None else ""
                    case OutputFormat.TXT:
                        content = result.text
                    case OutputFormat.JSON:
                        content = json.dumps(result.to_dict())
                    case _:
                        raise ValueError(f"Unsupported output format: {output_format}")

                # Translate if target language differs from detected language
                if target_language and target_language != detected_language:
                    content = self._translate_content(
                        content=content,
                        source_lang=detected_language,
                        target_lang=target_language,
                        output_format=output_format,
                    )

                return content

        except Exception:
            logger.error(
                "Failed to transcribe audio (task=%s, format=%s)",
                task,
                output_format.value,
                exc_info=True,
            )
            raise

    def _translate_content(
        self,
        content: str,
        source_lang: str,
        target_lang: str,
        output_format: OutputFormat,
    ) -> str:
        """Translate subtitle content to target language.

        Args:
            content: Subtitle content to translate
            source_lang: Source language code
            target_lang: Target language code
            output_format: Format of the content (affects translation method)

        Returns:
            Translated content
        """
        if not content or source_lang == target_lang:
            return content

        logger.info(
            "Translating subtitles from %s to %s",
            source_lang,
            target_lang,
        )

        try:
            translation_service = TranslationService(self.config)

            match output_format:
                case OutputFormat.SRT:
                    return translation_service.translate_srt_content(content, source_lang, target_lang)
                case OutputFormat.VTT:
                    # VTT is similar to SRT, convert timestamps if needed
                    # For now, use SRT translation which works for most cases
                    return translation_service.translate_srt_content(content, source_lang, target_lang)
                case OutputFormat.TXT:
                    return translation_service.translate_text(content, source_lang, target_lang)
                case OutputFormat.JSON:
                    # JSON format contains the full result, skip translation
                    logger.warning("Translation not supported for JSON format")
                    return content
                case _:
                    return content

        except Exception as e:
            logger.error(
                "Translation failed from %s to %s: %s",
                source_lang,
                target_lang,
                e,
                exc_info=True,
            )
            # Return original content on translation failure
            return content
