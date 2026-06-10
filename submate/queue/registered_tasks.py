# submate/queue/registered_tasks.py
"""Statically registered Huey tasks.

This module defines tasks using @huey.task() decorator at module level.
Tasks are registered when this module is imported, making them available
to both the server and worker processes.

IMPORTANT: Both server and worker must import this module for tasks to work.
"""

import logging
from typing import Any, Literal

from submate.queue.task_queue import get_huey

logger = logging.getLogger(__name__)

# Get the Huey instance - tasks will be registered to this
huey = get_huey()


@huey.task(retries=3, retry_delay=60)
def transcribe_audio_task(
    audio_bytes: bytes,
    language: str | None = None,
    task: Literal["transcribe", "translate"] = "transcribe",
    output_format: str = "srt",
    word_timestamps: bool = False,
    target_language: str | None = None,
) -> dict[str, Any]:
    """Transcribe audio bytes and return subtitle content.

    This is a statically registered task that can be executed by workers.
    Services are initialized when the task runs.

    Args:
        audio_bytes: Raw audio data to transcribe
        language: Optional source language hint (e.g., "en", "es")
        task: "transcribe" or "translate" (Whisper's translate is to English only)
        output_format: Output format ("srt", "vtt", "txt", "json")
        word_timestamps: Enable word-level timestamps
        target_language: Target language for subtitles. If different from
            transcribed language, translation will be performed using LLM.

    Returns:
        Dict with 'success', 'data' (subtitle content), and optionally 'error'
    """
    from submate.config import get_config
    from submate.queue.models import OutputFormat
    from submate.queue.services.bazarr import BazarrService

    logger.info(
        "Worker executing transcribe_audio_task: task=%s, language=%s, target=%s, format=%s",
        task,
        language,
        target_language,
        output_format,
    )

    try:
        config = get_config()
        bazarr_service = BazarrService(config)

        # Convert string to OutputFormat enum
        try:
            output_format_enum = OutputFormat(output_format)
        except ValueError:
            output_format_enum = OutputFormat.SRT

        subtitle_content = bazarr_service.transcribe_audio_bytes(
            audio_bytes=audio_bytes,
            language=language,
            task=task,
            output_format=output_format_enum,
            word_timestamps=word_timestamps,
            target_language=target_language,
        )

        logger.info("Worker completed transcribe_audio_task successfully")
        return {"success": True, "data": subtitle_content}

    except Exception as e:
        logger.error("Worker transcribe_audio_task failed: %s", e, exc_info=True)
        return {"success": False, "error": str(e), "data": None}


@huey.task(retries=2, retry_delay=30)
def detect_language_task(audio_bytes: bytes) -> dict[str, Any]:
    """Detect language from audio bytes.

    This is a statically registered task that can be executed by workers.

    Args:
        audio_bytes: Raw audio data for language detection

    Returns:
        Dict with 'success', 'data' (language info), and optionally 'error'
    """
    from submate.config import get_config
    from submate.queue.services.bazarr import BazarrService

    logger.info("Worker executing detect_language_task (%d bytes)", len(audio_bytes))

    try:
        config = get_config()
        bazarr_service = BazarrService(config)

        result = bazarr_service.detect_language(audio_bytes)

        logger.info(
            "Worker completed detect_language_task: %s (%s)",
            result.get("detected_language"),
            result.get("language_code"),
        )
        return {"success": True, "data": result}

    except Exception as e:
        logger.error("Worker detect_language_task failed: %s", e, exc_info=True)
        return {
            "success": False,
            "error": str(e),
            "data": {
                "detected_language": "Unknown",
                "language_code": "und",
            },
        }


@huey.task(retries=3, retry_delay=60)
def transcribe_file_task(
    file_path: str,
    audio_language: str | None = None,
    translate_to: str | None = None,
    force: bool = False,
) -> dict[str, Any]:
    """Transcribe a media file on disk.

    Statically registered so a separate worker process can find it in its
    registry and execute it. Only plain serializable arguments are accepted;
    the service is reconstructed from config inside the worker.

    Args:
        file_path: Path to the media file to transcribe
        audio_language: Optional source language hint (e.g., "en", "es")
        translate_to: Optional target language for LLM translation
        force: Re-transcribe even if subtitles already exist

    Returns:
        Dict with 'success', 'data' (TranscriptionResult), and optionally
        'error' or skip details.
    """
    from pathlib import Path

    from submate.config import get_config
    from submate.queue.models import TranscriptionSkippedError
    from submate.queue.services import TranscriptionService

    logger.info("Worker executing transcribe_file_task: %s", file_path)

    try:
        config = get_config()
        service = TranscriptionService(config)
        result = service.transcribe_file(Path(file_path), audio_language, translate_to, force)
        logger.info("Worker completed transcribe_file_task: %s", file_path)
        return {"success": True, "data": result}

    except TranscriptionSkippedError as e:
        # Skips are an expected outcome, not a failure to retry.
        logger.info("Transcription skipped for %s: %s", file_path, e.reason.value)
        return {"success": True, "skipped": True, "reason": e.reason.value, "data": None}

    except Exception as e:
        logger.error("Worker transcribe_file_task failed for %s: %s", file_path, e, exc_info=True)
        return {"success": False, "error": str(e), "data": None}


# Export the tasks so they can be called from handlers
__all__ = ["transcribe_audio_task", "detect_language_task", "transcribe_file_task", "huey"]
