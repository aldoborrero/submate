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
) -> dict[str, Any]:
    """Transcribe audio bytes and return subtitle content.

    This is a statically registered task that can be executed by workers.
    Services are initialized when the task runs.

    Args:
        audio_bytes: Raw audio data to transcribe
        language: Optional language code (e.g., "en", "es")
        task: "transcribe" or "translate"
        output_format: Output format ("srt", "vtt", "txt", "json")
        word_timestamps: Enable word-level timestamps

    Returns:
        Dict with 'success', 'data' (subtitle content), and optionally 'error'
    """
    from submate.config import get_config
    from submate.queue.models import OutputFormat
    from submate.queue.services.bazarr import BazarrService

    logger.info(
        "Worker executing transcribe_audio_task: task=%s, language=%s, format=%s",
        task,
        language,
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


# Export the tasks so they can be called from handlers
__all__ = ["transcribe_audio_task", "detect_language_task", "huey"]
