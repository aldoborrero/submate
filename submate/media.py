"""Media file utilities for audio extraction and manipulation."""

import logging
from dataclasses import dataclass
from io import BytesIO
from pathlib import Path

import ffmpeg

logger = logging.getLogger(__name__)


@dataclass
class AudioTrack:
    """Represents an audio track in a media file."""

    index: int
    language: str
    codec: str


def get_audio_tracks(video_path: Path) -> list[AudioTrack]:
    """Extract audio track information from a video file.

    Args:
        video_path: Path to the video file

    Returns:
        List of AudioTrack objects

    Raises:
        ffmpeg.Error: If ffprobe fails
    """
    try:
        probe = ffmpeg.probe(str(video_path), select_streams="a")
        tracks = []

        for idx, stream in enumerate(probe.get("streams", [])):
            language = stream.get("tags", {}).get("language", "und")
            codec = stream.get("codec_name", "unknown")
            tracks.append(AudioTrack(index=idx, language=language, codec=codec))

        return tracks

    except ffmpeg.Error as e:
        logger.error("ffprobe failed: %s", e.stderr.decode() if e.stderr else str(e), exc_info=True)
        raise


def get_audio_track_by_language(tracks: list[AudioTrack], language: str) -> AudioTrack | None:
    """Find an audio track by language code.

    Args:
        tracks: List of AudioTrack objects
        language: ISO 639-2/3 language code (case-insensitive)

    Returns:
        AudioTrack if found, None otherwise
    """
    if not tracks:
        return None

    language_lower = language.lower()
    for track in tracks:
        if track.language.lower() == language_lower:
            return track

    return None


def get_audio_languages(video_path: Path) -> list[str]:
    """Get all audio track languages from a video file.

    Args:
        video_path: Path to the video file

    Returns:
        List of language codes for each audio track
    """
    try:
        tracks = get_audio_tracks(video_path)
        return [track.language for track in tracks]
    except Exception as e:
        logger.debug("Failed to get audio languages from %s: %s", video_path, e)
        return []


def extract_audio_track_to_memory(
    video_path: Path,
    track_index: int = 0,
    format: str = "wav",
) -> BytesIO:
    """Extract an audio track from video to memory.

    Args:
        video_path: Path to the video file
        track_index: Index of the audio track to extract
        format: Output format (wav, mp3, etc.)

    Returns:
        BytesIO object containing the audio data

    Raises:
        ffmpeg.Error: If ffmpeg fails
    """
    try:
        # Use FFmpeg to extract the specific audio track and output to memory
        out, _ = (
            ffmpeg.input(str(video_path))
            .output(
                "pipe:",  # Direct output to a pipe
                map=f"0:a:{track_index}",  # Select the specific audio track
                format=format,  # Output format
                ac=1,  # Mono audio
                ar=16000,  # Sample rate 16 kHz (recommended for speech models)
                loglevel="quiet",
            )
            .run(capture_stdout=True, capture_stderr=True)  # Capture output in memory
        )
        # Return the audio data as a BytesIO object
        return BytesIO(out)

    except ffmpeg.Error as e:
        logger.error("ffmpeg failed: %s", e.stderr.decode(), exc_info=True)
        raise


def prepare_audio_for_transcription(
    file_path: Path,
    language: str | None = None,
) -> Path | BytesIO:
    """Prepare audio for transcription, extracting specific track only if needed.

    This function checks if the media file has multiple audio tracks. If it does,
    it extracts the appropriate track based on language preference. Otherwise,
    it returns the file path for direct processing by Whisper.

    Args:
        file_path: Path to the media file
        language: Optional language code to prefer when selecting audio track

    Returns:
        Either the original file Path (for single-track files) or BytesIO (for extracted multi-track audio)
    """
    try:
        audio_tracks = get_audio_tracks(file_path)

        # If only one track (or no tracks), return path directly
        if len(audio_tracks) <= 1:
            logger.debug("Single audio track detected, passing file path directly: %s", file_path)
            return file_path

        # Multiple tracks - need to extract specific one
        logger.debug("Multiple audio tracks detected (%d), extracting specific track", len(audio_tracks))

        # Try to find track by language if specified
        selected_track = None
        if language:
            selected_track = get_audio_track_by_language(audio_tracks, language)
            if selected_track:
                logger.debug("Selected audio track by language '%s': index %d", language, selected_track.index)

        # Fall back to first track if language match not found
        if selected_track is None:
            selected_track = audio_tracks[0]
            logger.debug("Using first audio track: index %d", selected_track.index)

        # Extract the specific track to memory
        audio_data = extract_audio_track_to_memory(file_path, selected_track.index)
        logger.debug("Extracted audio track %d to memory", selected_track.index)
        return audio_data

    except Exception as e:
        logger.warning("Failed to detect audio tracks, falling back to direct path: %s", e)
        # If anything fails, fall back to passing the file path directly
        return file_path
