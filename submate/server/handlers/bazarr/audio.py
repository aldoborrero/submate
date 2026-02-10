"""Audio utilities for Bazarr integration."""

import logging
from io import BytesIO

import ffmpeg

logger = logging.getLogger(__name__)


def extract_audio_segment(
    audio_file: BytesIO,
    offset: int = 0,
    length: int = 30,
) -> bytes:
    """Extract audio segment from uploaded file.

    Args:
        audio_file: Uploaded audio file
        offset: Start offset in seconds
        length: Duration in seconds

    Returns:
        Raw audio bytes (16-bit PCM, 16kHz)

    Raises:
        RuntimeError: If extraction fails
    """
    try:
        logger.debug("Extracting audio segment: offset=%ss, length=%ss", offset, length)

        # Read file content
        audio_file.seek(0)
        file_content = audio_file.read()

        # Use ffmpeg to extract segment
        process = (
            ffmpeg.input("pipe:", format="wav")
            .filter("atrim", start=offset, duration=length)
            .output("pipe:", format="s16le", acodec="pcm_s16le", ac=1, ar=16000)
            .run_async(pipe_stdin=True, pipe_stdout=True, pipe_stderr=True)
        )

        stdout, stderr = process.communicate(input=file_content)

        if process.returncode != 0:
            raise RuntimeError(f"ffmpeg failed: {stderr.decode()}")

        logger.debug("Extracted %d bytes", len(stdout))
        return bytes(stdout)

    except Exception as e:
        logger.error("Failed to extract audio segment: %s", e, exc_info=True)
        raise RuntimeError(f"Audio segment extraction failed: {e}") from e
