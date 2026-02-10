"""Path manipulation and validation utilities."""

from pathlib import Path

from submate.language import LanguageCode
from submate.types import LanguageNamingType

VIDEO_EXTENSIONS = {
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
    ".3gp",
    ".ogv",
}

AUDIO_EXTENSIONS = {
    ".mp3",
    ".flac",
    ".aac",
    ".m4a",
    ".wav",
    ".ogg",
    ".opus",
    ".wma",
    ".alac",
    ".ape",
}


def map_path(
    path: str | Path,
    use_mapping: bool,
    path_from: str,
    path_to: str,
) -> str:
    """Map a path from one location to another.

    This is useful for Docker containers where the host path
    differs from the container path.

    Args:
        path: Path to map
        use_mapping: Whether to perform mapping
        path_from: Source path prefix to replace
        path_to: Destination path prefix

    Returns:
        Mapped path as string
    """
    path_str = str(path)

    if not use_mapping:
        return path_str

    if not path_from or not path_to:
        return path_str

    if path_str.startswith(path_from):
        return path_str.replace(path_from, path_to, 1)

    return path_str


def format_language_for_filename(
    language: str | LanguageCode | None,
    naming_type: LanguageNamingType = LanguageNamingType.ISO_639_2_B,
) -> str:
    """Format a language code according to the naming type.

    Args:
        language: Language code string or LanguageCode enum
        naming_type: How to format the language in the filename

    Returns:
        Formatted language string, or empty string if language is None/invalid
    """
    if not language:
        return ""

    # Convert string to LanguageCode if needed
    if isinstance(language, str):
        lang_code = LanguageCode.from_string(language)
    else:
        lang_code = language

    if lang_code is LanguageCode.NONE:
        # Fall back to original string if we can't parse it
        return language if isinstance(language, str) else ""

    match naming_type:
        case LanguageNamingType.ISO_639_1:
            return lang_code.to_iso_639_1() or ""
        case LanguageNamingType.ISO_639_2_T:
            return lang_code.to_iso_639_2_t() or ""
        case LanguageNamingType.ISO_639_2_B:
            return lang_code.to_iso_639_2_b() or ""
        case LanguageNamingType.NAME:
            return lang_code.to_name(in_english=True) or ""
        case LanguageNamingType.NATIVE:
            return lang_code.to_name(in_english=False) or ""
        case _:
            return lang_code.to_iso_639_2_b() or ""


def build_subtitle_path(
    video_path: str | Path,
    language: str | LanguageCode | None = None,
    naming_type: LanguageNamingType = LanguageNamingType.ISO_639_2_B,
    include_subgen_marker: bool = False,
    include_model: bool = False,
    model_name: str = "",
    extension: str = ".srt",
) -> str:
    """Build a subtitle file path with configurable naming options.

    Args:
        video_path: Path to the video file
        language: Language code (string or LanguageCode enum)
        naming_type: How to format the language in the filename
        include_subgen_marker: Include .subgen in filename
        include_model: Include model name in filename
        model_name: Whisper model name (e.g., 'medium', 'large-v3')
        extension: Subtitle file extension (default: .srt)

    Returns:
        Full path to the subtitle file

    Examples:
        >>> build_subtitle_path("movie.mp4", "eng")
        'movie.eng.srt'

        >>> build_subtitle_path("movie.mp4", "eng", include_subgen_marker=True)
        'movie.subgen.eng.srt'

        >>> build_subtitle_path("movie.mp4", "eng", include_model=True, model_name="medium")
        'movie.medium.eng.srt'

        >>> build_subtitle_path("movie.mp4", "eng", naming_type=LanguageNamingType.ISO_639_1)
        'movie.en.srt'
    """
    path = Path(video_path)
    stem = path.stem
    parent = path.parent

    # Build the parts list
    parts = [stem]

    # Add subgen marker if requested
    if include_subgen_marker:
        parts.append("subgen")

    # Add model name if requested
    if include_model and model_name:
        parts.append(model_name)

    # Add formatted language
    formatted_lang = format_language_for_filename(language, naming_type)
    if formatted_lang:
        parts.append(formatted_lang)

    # Ensure extension starts with dot
    if not extension.startswith("."):
        extension = f".{extension}"

    # Build final filename
    subtitle_name = ".".join(parts) + extension

    return str(parent / subtitle_name)


def get_subtitle_path(video_path: str | Path, language: str = "") -> str:
    """Generate the subtitle file path for a video (simple version).

    For more options, use build_subtitle_path().

    Args:
        video_path: Path to the video file
        language: Optional language code (e.g., 'eng', 'spa')

    Returns:
        Path to the subtitle file
    """
    return build_subtitle_path(video_path, language)


def is_video_file(path: str | Path) -> bool:
    """Check if a file is a video file based on extension.

    Args:
        path: File path to check

    Returns:
        True if file is a video file
    """
    return Path(path).suffix.lower() in VIDEO_EXTENSIONS


def is_audio_file(path: str | Path) -> bool:
    """Check if a file is an audio file based on extension.

    Args:
        path: File path to check

    Returns:
        True if file is an audio file
    """
    return Path(path).suffix.lower() in AUDIO_EXTENSIONS
