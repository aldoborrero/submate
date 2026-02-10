"""Bazarr webhook handlers for ASR and language detection."""

import logging
from io import BytesIO

from submate.queue import get_task_queue
from submate.queue.tasks import BazarrTranscriptionTask, LanguageDetectionTask
from submate.server.handlers.bazarr.audio import extract_audio_segment
from submate.server.handlers.bazarr.models import LanguageDetectionResponse

logger = logging.getLogger(__name__)


async def handle_asr_request(
    audio_file: BytesIO,
    task: str = "transcribe",
    language: str | None = None,
    output: str = "srt",
    encode: bool = True,
    word_timestamps: bool = False,
    video_file: str | None = None,
) -> str:
    """Handle Bazarr ASR transcription request via Huey queue.

    Enqueues the transcription task and blocks waiting for results.
    This ensures proper concurrency control while maintaining synchronous
    behavior from Bazarr's perspective.

    Args:
        audio_file: Uploaded audio file
        task: "transcribe" or "translate"
        language: Optional language code
        output: Output format (srt, vtt, txt, json)
        encode: Ignored (Bazarr sends encode=false after pre-encoding with ffmpeg)
        word_timestamps: Enable word-level timestamps
        video_file: Optional filename for logging

    Returns:
        Subtitle content as string

    Raises:
        ValueError: If invalid output format
        RuntimeError: If transcription fails
    """
    # Note: encode=false means Bazarr pre-encoded the audio with ffmpeg
    # We accept both - the audio is already in a usable format either way
    _ = encode  # Unused but accepted for Bazarr compatibility

    if output not in ("srt", "vtt", "txt", "json"):
        raise ValueError(f"Invalid output format: {output}")

    logger.info(
        f"{task.capitalize()} of file '{video_file}' from Bazarr" if video_file else f"{task.capitalize()} from Bazarr"
    )

    try:
        # Read audio content
        audio_file.seek(0)
        audio_content = audio_file.read()

        # Enqueue transcription via Huey and wait for result
        # This blocks until the worker completes the task
        task_queue = get_task_queue()
        result = task_queue.enqueue(
            BazarrTranscriptionTask,
            audio_bytes=audio_content,
            language=language,
            task=task,  # Pass transcribe/translate mode
            output_format=output,
            word_timestamps=word_timestamps,
            blocking=True,  # Block until complete
        )

        logger.info(f"{task.capitalize()} complete for Bazarr request")
        return str(result)

    except Exception as e:
        logger.error(f"ASR request failed: {e}", exc_info=True)
        raise RuntimeError(f"Transcription failed: {e}") from e


async def handle_detect_language(
    audio_file: BytesIO,
    offset: int = 0,
    length: int = 30,
    video_file: str | None = None,
) -> LanguageDetectionResponse:
    """Handle Bazarr language detection request via Huey queue.

    Extracts audio segment and enqueues detection task, blocking for results.

    Args:
        audio_file: Uploaded audio file
        offset: Start offset in seconds
        length: Duration to analyze in seconds
        video_file: Optional filename for logging

    Returns:
        Language detection response
    """
    logger.info(
        f"Detecting language for '{video_file}' from Bazarr" if video_file else "Detecting language from Bazarr"
    )

    try:
        # Extract audio segment
        segment_data = extract_audio_segment(audio_file, offset=offset, length=length)

        # Enqueue detection via Huey and wait for result
        task_queue = get_task_queue()
        result_dict = task_queue.enqueue(
            LanguageDetectionTask,
            audio_bytes=segment_data,
            blocking=True,
        )

        return LanguageDetectionResponse(**result_dict)

    except Exception as e:
        logger.error(f"Language detection failed: {e}", exc_info=True)
        # Return unknown instead of failing
        return LanguageDetectionResponse(
            detected_language="Unknown",
            language_code="und",
        )
