# submate/queue/services/transcription.py
import logging
from pathlib import Path

import srt

from submate.config import Config
from submate.language import LanguageCode
from submate.media import get_audio_languages, prepare_audio_for_transcription
from submate.paths import build_subtitle_path, is_audio_file
from submate.subtitle import (
    get_internal_subtitle_languages,
    has_any_external_subtitle,
    has_internal_subtitle_language,
    has_lrc_file,
    has_subtitle_language,
)
from submate.translation import TranslationService
from submate.whisper import WhisperModelWrapper

from ..models import SkipReason, TranscriptionResult, TranscriptionSkippedError

logger = logging.getLogger(__name__)


class TranscriptionService:
    """Handles file-based transcription operations."""

    def __init__(self, config: Config):
        self.config = config

    def transcribe_file(
        self, file_path: Path, audio_language: str | None, translate_to: str | None, force: bool
    ) -> TranscriptionResult:
        """Transcribe a file to subtitles, optionally translating to another language.

        Args:
            file_path: Path to the media file
            audio_language: Language code to select specific audio track (e.g., 'ja' for Japanese)
            translate_to: Target language for translation (e.g., 'es', 'fr'). If None, no translation.
            force: Skip all skip conditions
        """
        # For skip logic, target language is translation target if set, otherwise audio language or auto
        target_language = (
            LanguageCode.from_string(translate_to) if translate_to else LanguageCode.from_string(audio_language)
        )

        # Validate LLM backend early if non-English translation is requested
        self.config.translation.validate_for_target(translate_to)

        # Check skip conditions
        skip, reason = self._should_skip_transcription(file_path, target_language)
        if skip and not force:
            raise TranscriptionSkippedError(reason)

        # Determine if we should use Whisper's built-in translation (only works for → English)
        use_whisper_translate = translate_to and LanguageCode.from_string(translate_to) == LanguageCode.ENGLISH

        with WhisperModelWrapper(self.config) as model:
            audio = prepare_audio_for_transcription(file_path, audio_language)

            if use_whisper_translate:
                # Use Whisper's built-in translation to English (free, no LLM API needed)
                logger.info("Using Whisper's built-in translation to English")
                result = model.transcribe(audio, language=audio_language, task="translate")
            else:
                # Transcribe only - LLM will handle translation to other languages
                result = model.transcribe(audio, language=audio_language, task="transcribe")

            # Determine source language (detected by Whisper)
            source_language = audio_language or result.language
            subtitle_settings = self.config.subtitle

            # Determine final output language
            if subtitle_settings.force_detected_language_to:
                output_language = subtitle_settings.force_detected_language_to
                logger.debug(f"Forcing output language to: {output_language}")
            elif translate_to:
                output_language = translate_to
            else:
                output_language = source_language

            # Build subtitle path with naming options from config
            subtitle_path = build_subtitle_path(
                file_path,
                language=output_language,
                naming_type=subtitle_settings.language_naming_type,
                include_subgen_marker=subtitle_settings.include_subgen_marker,
                include_model=subtitle_settings.include_model_in_filename,
                model_name=self.config.whisper.model,
            )

            # Write initial SRT file
            result.to_srt_vtt(subtitle_path, word_level=self.config.stable_ts.word_level_highlight)

            # Post-transcription LLM translation (only for non-English targets)
            final_text = result.text
            if translate_to and not use_whisper_translate and translate_to != source_language:
                logger.info(f"Translating subtitles from {source_language} to {translate_to} via LLM")
                translation_service = TranslationService(self.config)

                # Read the SRT file, translate, and write back
                with open(subtitle_path, encoding="utf-8") as f:
                    srt_content = f.read()

                translated_content = translation_service.translate_srt_content(
                    srt_content, source_language, translate_to
                )

                with open(subtitle_path, "w", encoding="utf-8") as f:
                    f.write(translated_content)

                # Extract translated text for result
                translated_subs = list(srt.parse(translated_content))
                final_text = "\n".join(sub.content for sub in translated_subs)

            return TranscriptionResult(
                subtitle_path=subtitle_path,
                language=output_language,
                segments=len(result.segments),
                text=final_text,
            )

    def _should_skip_transcription(
        self,
        file_path: Path,
        target_language: LanguageCode | None,
    ) -> tuple[bool, SkipReason]:
        """Check if transcription should be skipped.

        Evaluates skip conditions in priority order:
        1. LRC file exists (for audio files)
        2. Unknown language and skip_unknown_language is enabled
        3. Target subtitle already exists
        4. Internal subtitle in specific language exists
        5. External subtitle exists
        6. Subtitle language in skip list
        7. Audio language in skip list
        8. No preferred audio language found
        9. Language not set but subtitles exist

        Args:
            file_path: Path to the media file
            target_language: Target language for transcription (may be None)

        Returns:
            Tuple of (should_skip, skip_reason)
        """
        settings = self.config.subtitle

        # Condition 1: LRC file exists for audio files
        if is_audio_file(file_path) and settings.lrc_for_audio_files:
            if has_lrc_file(file_path):
                logger.debug(f"Skipping {file_path.name}: LRC file already exists")
                return True, SkipReason.LRC_FILE_EXISTS

        # Condition 2: Unknown language
        if settings.skip_unknown_language and target_language is None:
            logger.debug(f"Skipping {file_path.name}: Unknown language and skip_unknown_language enabled")
            return True, SkipReason.UNKNOWN_LANGUAGE

        # Condition 3: Target subtitle exists
        if settings.skip_if_target_subtitle_exists and target_language:
            if has_subtitle_language(
                file_path,
                target_language,
                only_subgen=settings.only_skip_if_subgen_subtitle,
            ):
                logger.debug(f"Skipping {file_path.name}: Subtitle already exists in {target_language}")
                return True, SkipReason.TARGET_SUBTITLE_EXISTS

        # Condition 4: Internal subtitle in specific language exists
        if settings.skip_if_internal_subtitle_language:
            skip_lang = LanguageCode.from_string(settings.skip_if_internal_subtitle_language)
            if skip_lang and has_internal_subtitle_language(file_path, skip_lang):
                logger.debug(f"Skipping {file_path.name}: Internal subtitle in {skip_lang} exists")
                return True, SkipReason.INTERNAL_SUBTITLE_LANGUAGE_EXISTS

        # Condition 5: External subtitle exists
        if settings.skip_if_external_subtitles_exist:
            if has_any_external_subtitle(file_path):
                logger.debug(f"Skipping {file_path.name}: External subtitle file exists")
                return True, SkipReason.EXTERNAL_SUBTITLE_EXISTS

        # Condition 6: Subtitle language in skip list
        if settings.skip_subtitle_languages:
            skip_langs = {LanguageCode.from_string(lang) for lang in settings.skip_subtitle_languages}
            skip_langs.discard(LanguageCode.NONE)  # Remove invalid codes
            internal_langs = set(get_internal_subtitle_languages(file_path))
            if skip_langs & internal_langs:  # Set intersection
                matched = skip_langs & internal_langs
                logger.debug(f"Skipping {file_path.name}: Contains skipped subtitle language {matched}")
                return True, SkipReason.SUBTITLE_LANGUAGE_IN_SKIP_LIST

        # Condition 7: Audio language in skip list
        if settings.skip_if_audio_languages:
            skip_audio_langs = {LanguageCode.from_string(lang) for lang in settings.skip_if_audio_languages}
            skip_audio_langs.discard(LanguageCode.NONE)
            audio_lang_strs = get_audio_languages(file_path)
            audio_langs = {LanguageCode.from_string(lang) for lang in audio_lang_strs}
            if skip_audio_langs & audio_langs:
                matched = skip_audio_langs & audio_langs
                logger.debug(f"Skipping {file_path.name}: Contains skipped audio language {matched}")
                return True, SkipReason.AUDIO_LANGUAGE_IN_SKIP_LIST

        # Condition 8: No preferred audio language found
        if settings.limit_to_preferred_audio_languages and settings.preferred_audio_languages:
            preferred = {LanguageCode.from_string(lang) for lang in settings.preferred_audio_languages}
            preferred.discard(LanguageCode.NONE)
            audio_lang_strs = get_audio_languages(file_path)
            audio_langs = {LanguageCode.from_string(lang) for lang in audio_lang_strs}
            if not (preferred & audio_langs):
                logger.debug(f"Skipping {file_path.name}: No preferred audio language found")
                return True, SkipReason.NO_PREFERRED_AUDIO_LANGUAGE

        # Condition 9: Language not set but subtitles exist
        if settings.skip_if_no_language_but_subtitles_exist:
            if target_language is None:
                existing_internal_langs = get_internal_subtitle_languages(file_path)
                if existing_internal_langs:
                    logger.debug(f"Skipping {file_path.name}: Language not set but internal subtitles exist")
                    return True, SkipReason.LANGUAGE_NOT_SET_BUT_SUBTITLES_EXIST

        logger.debug(f"No skip conditions met for {file_path.name}")
        return False, SkipReason.NOT_SKIPPED
