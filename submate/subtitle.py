"""Subtitle detection utilities for skip logic.

Provides functions to detect internal (embedded) and external (file-based)
subtitles for a given video file.
"""

import logging
from pathlib import Path

from submate.language import LanguageCode

logger = logging.getLogger(__name__)

# Supported subtitle file extensions
SUBTITLE_EXTENSIONS = {".srt", ".vtt", ".sub", ".ass", ".ssa", ".idx", ".sbv", ".pgs", ".ttml", ".lrc"}


def get_internal_subtitle_languages(file_path: Path) -> list[LanguageCode]:
    """Get languages of internal (embedded) subtitles using PyAV.

    Args:
        file_path: Path to the video file

    Returns:
        List of LanguageCode for each subtitle stream
    """
    try:
        import av

        languages = []
        with av.open(str(file_path)) as container:
            for stream in container.streams:
                if stream.type == "subtitle":
                    lang_code = stream.metadata.get("language", "")
                    languages.append(LanguageCode.from_iso_639_2(lang_code) or LanguageCode.NONE)
        return languages
    except Exception as e:
        logger.debug(f"Failed to read internal subtitles from {file_path}: {e}")
        return []


def get_external_subtitle_paths(video_path: Path) -> list[Path]:
    """Find external subtitle files for a video.

    Looks for subtitle files that match the video filename pattern
    (e.g., movie.en.srt, movie.english.srt for movie.mp4).

    Args:
        video_path: Path to the video file

    Returns:
        List of paths to matching subtitle files
    """
    if not video_path.exists():
        return []

    video_dir = video_path.parent
    video_stem = video_path.stem  # filename without extension

    subtitle_paths = []
    try:
        for file in video_dir.iterdir():
            if not file.is_file():
                continue
            if file.suffix.lower() not in SUBTITLE_EXTENSIONS:
                continue
            # Check if subtitle filename starts with video name
            if file.stem.startswith(video_stem):
                subtitle_paths.append(file)
    except OSError as e:
        logger.debug(f"Failed to scan directory {video_dir}: {e}")

    return subtitle_paths


def parse_subtitle_language(subtitle_path: Path, video_stem: str) -> LanguageCode:
    """Parse language from subtitle filename.

    Extracts language from subtitle filename patterns like:
    - movie.en.srt -> English
    - movie.eng.srt -> English
    - movie.english.srt -> English
    - movie.subgen.medium.en.srt -> English

    Args:
        subtitle_path: Path to the subtitle file
        video_stem: Video filename without extension

    Returns:
        LanguageCode parsed from filename, or NONE if not found
    """
    # Get the part after the video name
    subtitle_stem = subtitle_path.stem
    if not subtitle_stem.startswith(video_stem):
        return LanguageCode.NONE

    # Extract parts after video name (e.g., ".en" or ".subgen.medium.en")
    suffix = subtitle_stem[len(video_stem) :].lstrip(".")
    if not suffix:
        return LanguageCode.NONE

    # Try each part as a potential language code
    parts = suffix.split(".")
    for part in parts:
        lang = LanguageCode.from_string(part)
        if lang is not LanguageCode.NONE:
            return lang

    return LanguageCode.NONE


def has_internal_subtitle_language(video_path: Path, language: LanguageCode) -> bool:
    """Check if video has internal subtitle in specified language.

    Args:
        video_path: Path to the video file
        language: Language to check for

    Returns:
        True if internal subtitle in language exists
    """
    internal_langs = get_internal_subtitle_languages(video_path)
    return language in internal_langs


def has_external_subtitle_language(
    video_path: Path,
    language: LanguageCode,
    only_subgen: bool = False,
) -> bool:
    """Check if video has external subtitle in specified language.

    Args:
        video_path: Path to the video file
        language: Language to check for
        only_subgen: If True, only consider subtitles with 'subgen' in filename

    Returns:
        True if external subtitle in language exists
    """
    subtitle_paths = get_external_subtitle_paths(video_path)
    video_stem = video_path.stem

    for sub_path in subtitle_paths:
        # Check for subgen marker if required
        if only_subgen and "subgen" not in sub_path.stem.lower():
            continue

        # Parse language from filename
        sub_lang = parse_subtitle_language(sub_path, video_stem)
        if sub_lang == language:
            return True

    return False


def has_subtitle_language(
    video_path: Path,
    language: LanguageCode,
    only_subgen: bool = False,
) -> bool:
    """Check if video has subtitle (internal OR external) in specified language.

    Args:
        video_path: Path to the video file
        language: Language to check for
        only_subgen: If True, only consider external subtitles with 'subgen' in filename

    Returns:
        True if subtitle in language exists (internal or external)
    """
    # Check internal subtitles first (these can't be "subgen")
    if not only_subgen and has_internal_subtitle_language(video_path, language):
        return True

    # Check external subtitles
    return has_external_subtitle_language(video_path, language, only_subgen=only_subgen)


def has_any_external_subtitle(video_path: Path) -> bool:
    """Check if video has any external subtitle files.

    Args:
        video_path: Path to the video file

    Returns:
        True if any external subtitle file exists
    """
    return len(get_external_subtitle_paths(video_path)) > 0


def has_any_internal_subtitle(video_path: Path) -> bool:
    """Check if video has any internal (embedded) subtitles.

    Args:
        video_path: Path to the video file

    Returns:
        True if any internal subtitle stream exists
    """
    return len(get_internal_subtitle_languages(video_path)) > 0


def get_lrc_path(audio_path: Path) -> Path:
    """Get the LRC file path for an audio file.

    Args:
        audio_path: Path to the audio file

    Returns:
        Path to the corresponding LRC file (may not exist)
    """
    return audio_path.with_suffix(".lrc")


def has_lrc_file(audio_path: Path) -> bool:
    """Check if an LRC file exists for an audio file.

    Args:
        audio_path: Path to the audio file

    Returns:
        True if LRC file exists
    """
    return get_lrc_path(audio_path).exists()
