"""Tests for path utilities."""

from pathlib import Path

from submate.paths import (
    get_subtitle_path,
    is_audio_file,
    is_video_file,
    map_path,
)


def test_map_path_enabled() -> None:
    """Test path mapping when enabled."""
    result = map_path(
        "/host/media/movies/film.mp4",
        use_mapping=True,
        path_from="/host/media",
        path_to="/container/media",
    )
    assert result == "/container/media/movies/film.mp4"


def test_map_path_disabled() -> None:
    """Test path mapping when disabled."""
    result = map_path(
        "/host/media/movies/film.mp4",
        use_mapping=False,
        path_from="/host/media",
        path_to="/container/media",
    )
    assert result == "/host/media/movies/film.mp4"


def test_map_path_no_match() -> None:
    """Test path mapping when path doesn't match."""
    result = map_path(
        "/other/location/film.mp4",
        use_mapping=True,
        path_from="/host/media",
        path_to="/container/media",
    )
    assert result == "/other/location/film.mp4"


def test_map_path_with_pathlib() -> None:
    """Test path mapping with Path objects."""
    result = map_path(
        Path("/host/media/movies/film.mp4"),
        use_mapping=True,
        path_from="/host/media",
        path_to="/container/media",
    )
    assert result == "/container/media/movies/film.mp4"


def test_get_subtitle_path() -> None:
    """Test generating subtitle file path."""
    video_path = "/media/movies/film.mp4"
    subtitle_path = get_subtitle_path(video_path, language="eng")
    assert subtitle_path == "/media/movies/film.eng.srt"


def test_get_subtitle_path_no_language() -> None:
    """Test generating subtitle path without language."""
    video_path = "/media/movies/film.mkv"
    subtitle_path = get_subtitle_path(video_path)
    assert subtitle_path == "/media/movies/film.srt"


def test_get_subtitle_path_pathlib() -> None:
    """Test subtitle path generation with Path object."""
    video_path = Path("/media/movies/film.mp4")
    subtitle_path = get_subtitle_path(video_path, language="spa")
    assert subtitle_path == "/media/movies/film.spa.srt"


def test_is_video_file() -> None:
    """Test video file detection."""
    assert is_video_file("movie.mp4") is True
    assert is_video_file("movie.mkv") is True
    assert is_video_file("movie.avi") is True
    assert is_video_file("movie.MP4") is True  # Case insensitive
    assert is_video_file("movie.txt") is False
    assert is_video_file("movie.srt") is False
    assert is_video_file(Path("movie.mp4")) is True


def test_is_audio_file() -> None:
    """Test audio file detection."""
    assert is_audio_file("song.mp3") is True
    assert is_audio_file("song.flac") is True
    assert is_audio_file("song.aac") is True
    assert is_audio_file("song.MP3") is True  # Case insensitive
    assert is_audio_file("song.txt") is False
    assert is_audio_file("song.mp4") is False
    assert is_audio_file(Path("song.mp3")) is True
