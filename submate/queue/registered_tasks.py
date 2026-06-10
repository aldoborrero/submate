# submate/queue/registered_tasks.py
"""Statically registered Huey tasks.

This module defines tasks using @huey.task() decorator at module level.
Tasks are registered when this module is imported, making them available
to both the server and worker processes.

IMPORTANT: Both server and worker must import this module for tasks to work.
"""

import logging
import threading
from typing import Any, Literal

from submate.config import get_config
from submate.queue.task_queue import get_huey

logger = logging.getLogger(__name__)

# Get the Huey instance - tasks will be registered to this
huey = get_huey()

# Retry behavior is config-driven so operators can tune it via
# SUBMATE__QUEUE__MAX_RETRIES / SUBMATE__QUEUE__RETRY_DELAY. Read once at import
# time, matching how the worker process resolves its registry on startup.
_queue_settings = get_config().queue

# In-flight file transcriptions in this worker process, keyed by the full task
# parameters. Guards against a duplicate enqueue (e.g. Jellyfin firing ItemAdded
# twice) re-running the same transcription on another worker thread while the
# first is still in progress. In-memory by design: a crash clears it, so it can
# never wedge a file the way a persisted cross-process lock would.
_inflight_lock = threading.Lock()
_inflight_tasks: set[tuple[Any, ...]] = set()


# Synchronous Bazarr request: the server blocks on this result, so retrying here
# would make the client wait through every attempt -- and keep running expensive
# transcription for a client that may have already disconnected. Fail fast and
# let Bazarr re-request. (transcribe_file_task, which is fire-and-forget, does
# use the configured retries.)
@huey.task(retries=0)
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
        output_format_enum = OutputFormat.from_value(output_format)

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


# Synchronous, best-effort detection (the handler falls back to "Unknown" on
# failure), so there is no point retrying behind the waiting client. See
# transcribe_audio_task.
@huey.task(retries=0)
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


@huey.task(retries=_queue_settings.max_retries, retry_delay=_queue_settings.retry_delay)
def transcribe_file_task(
    file_path: str,
    audio_language: str | None = None,
    translate_to: str | None = None,
    force: bool = False,
) -> None:
    """Transcribe a media file on disk (fire-and-forget).

    Statically registered so a separate worker process can find it in its
    registry and execute it. Only plain serializable arguments are accepted;
    the service is reconstructed from config inside the worker.

    Returns ``None`` by design: this task is always dispatched fire-and-forget
    (Jellyfin webhooks and queued CLI runs discard the handle; ``--sync`` runs
    inline without Huey). A ``None`` return means Huey stores no result row
    (store_none=False), so the SQLite result store doesn't grow unbounded with
    results nobody reads. Every outcome is recorded in the worker log instead.

    A genuine failure is re-raised so Huey retries it (per the configured
    retries/retry_delay); Huey only stores an Error result once retries are
    exhausted -- bounded to permanently-failed jobs. Skips and successes return
    None and store nothing.

    Args:
        file_path: Path to the media file to transcribe
        audio_language: Optional source language hint (e.g., "en", "es")
        translate_to: Optional target language for LLM translation
        force: Re-transcribe even if subtitles already exist
    """
    from pathlib import Path

    from submate.config import get_config
    from submate.queue.models import TranscriptionSkippedError
    from submate.queue.services import TranscriptionService

    logger.info("Worker executing transcribe_file_task: %s", file_path)

    # Drop a duplicate that is already being transcribed in this process. The
    # original run writes the subtitle atomically; a later retry/re-enqueue will
    # then hit the "subtitle exists" skip instead of redoing the work.
    inflight_key = (file_path, audio_language, translate_to, force)
    with _inflight_lock:
        if inflight_key in _inflight_tasks:
            logger.info("Skipping duplicate in-flight transcription of %s", file_path)
            return None
        _inflight_tasks.add(inflight_key)

    try:
        config = get_config()
        service = TranscriptionService(config)
        result = service.transcribe_file(Path(file_path), audio_language, translate_to, force)
        logger.info("Worker completed transcribe_file_task: %s -> %s", file_path, result.subtitle_path)

    except TranscriptionSkippedError as e:
        # Skips are an expected outcome, not a failure to retry.
        logger.info("Transcription skipped for %s: %s", file_path, e.reason.value)

    except Exception as e:
        # Re-raise so Huey retries (retries/retry_delay are config-driven). The
        # in-flight marker is cleared by the finally below before the retry runs.
        logger.error("Worker transcribe_file_task failed for %s: %s", file_path, e, exc_info=True)
        raise

    finally:
        with _inflight_lock:
            _inflight_tasks.discard(inflight_key)


# Export the tasks so they can be called from handlers
__all__ = ["transcribe_audio_task", "detect_language_task", "transcribe_file_task", "huey"]
