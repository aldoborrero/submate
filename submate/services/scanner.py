"""Subtitle file scanner service.

Scans for subtitle files next to media files and detects their languages
from filename patterns.
"""

import logging
from pathlib import Path

from submate.language import LanguageCode

logger = logging.getLogger(__name__)

# Supported subtitle file extensions
SUBTITLE_EXTENSIONS: set[str] = {".srt", ".ass", ".ssa", ".sub", ".vtt"}

# Common video file extensions for media detection
MEDIA_EXTENSIONS: set[str] = {
    ".mp4",
    ".mkv",
    ".avi",
    ".mov",
    ".wmv",
    ".flv",
    ".webm",
    ".m4v",
    ".mpg",
    ".mpeg",
    ".ts",
    ".m2ts",
}

# Mapping of language identifiers to ISO 639-1 codes
# Includes ISO 639-1, ISO 639-2/T, ISO 639-2/B, and full language names
LANGUAGE_CODES: dict[str, str] = {}

# Build the language codes mapping from LanguageCode enum
for lang in LanguageCode:
    if lang is LanguageCode.NONE:
        continue
    iso1 = lang.iso_639_1
    if iso1:
        # Add ISO 639-1 code
        LANGUAGE_CODES[iso1] = iso1
        # Add ISO 639-2/T code
        if lang.iso_639_2_t:
            LANGUAGE_CODES[lang.iso_639_2_t] = iso1
        # Add ISO 639-2/B code (if different from T)
        if lang.iso_639_2_b and lang.iso_639_2_b != lang.iso_639_2_t:
            LANGUAGE_CODES[lang.iso_639_2_b] = iso1
        # Add English name (lowercase)
        if lang.name_en:
            LANGUAGE_CODES[lang.name_en.lower()] = iso1


class SubtitleScanner:
    """Scans for subtitle files and detects their languages."""

    def detect_language_from_filename(self, filename: str) -> str | None:
        """Detect language from filename patterns.

        Supports patterns like:
        - movie.en.srt (ISO 639-1)
        - movie.eng.srt (ISO 639-2)
        - movie.english.srt (full name)

        Args:
            filename: The filename to analyze

        Returns:
            ISO 639-1 language code (e.g., "en", "es") or None if not detected
        """
        # Remove the extension to get the base name
        path = Path(filename)
        stem = path.stem

        # Split by dots and check each part
        parts = stem.split(".")
        if len(parts) < 2:
            return None

        # Check each part from right to left (language code is usually at the end)
        for part in reversed(parts[1:]):  # Skip the first part (actual filename)
            part_lower = part.lower()
            if part_lower in LANGUAGE_CODES:
                return LANGUAGE_CODES[part_lower]

        return None

    def scan_for_media(self, media_path: Path) -> list[dict]:
        """Find subtitle files next to a media file.

        Args:
            media_path: Path to the media file

        Returns:
            List of dicts with keys: language, path, source
        """
        if not media_path.exists():
            logger.warning("Media file does not exist: %s", media_path)
            return []

        parent = media_path.parent
        media_stem = media_path.stem
        subtitles = []

        # Find all subtitle files that start with the media filename
        for ext in SUBTITLE_EXTENSIONS:
            # Match exact name: movie.srt
            exact_match = parent / f"{media_stem}{ext}"
            if exact_match.exists():
                lang = self.detect_language_from_filename(exact_match.name)
                subtitles.append(
                    {
                        "language": lang if lang else "und",
                        "path": exact_match,
                        "source": "external",
                    }
                )

            # Match with language codes: movie.en.srt, movie.eng.srt, movie.english.srt
            for sub_file in parent.glob(f"{media_stem}.*{ext}"):
                if sub_file == exact_match:
                    continue  # Already processed
                lang = self.detect_language_from_filename(sub_file.name)
                subtitles.append(
                    {
                        "language": lang if lang else "und",
                        "path": sub_file,
                        "source": "external",
                    }
                )

        logger.debug("Found %d subtitle files for %s", len(subtitles), media_path)
        return subtitles

    def scan_directory(self, directory: Path) -> dict[str, list[dict]]:
        """Scan directory for media files and their subtitles.

        Args:
            directory: Directory to scan

        Returns:
            Dict mapping media paths (as strings) to lists of subtitle dicts
        """
        if not directory.exists() or not directory.is_dir():
            logger.warning("Directory does not exist or is not a directory: %s", directory)
            return {}

        result: dict[str, list[dict]] = {}

        # Find all media files recursively
        for media_ext in MEDIA_EXTENSIONS:
            for media_file in directory.rglob(f"*{media_ext}"):
                media_path_str = str(media_file)
                if media_path_str in result:
                    continue  # Already processed

                subtitles = self.scan_for_media(media_file)
                if subtitles:
                    result[media_path_str] = subtitles

        logger.debug("Found %d media files with subtitles in %s", len(result), directory)
        return result
