"""Tests for media utilities."""

import pytest

from submate.media import (
    AudioTrack,
    get_audio_track_by_language,
)


def test_audio_track_dataclass():
    """Test AudioTrack dataclass."""
    track = AudioTrack(index=0, language="eng", codec="aac")
    assert track.index == 0
    assert track.language == "eng"
    assert track.codec == "aac"


@pytest.mark.skip(reason="Requires ffmpeg and real media files")
def test_get_audio_tracks_real_file():
    """Test extracting audio tracks from real file."""
    # This would require a real media file
    pass


def test_get_audio_track_by_language():
    """Test selecting audio track by language."""
    tracks = [
        AudioTrack(index=0, language="eng", codec="aac"),
        AudioTrack(index=1, language="spa", codec="aac"),
        AudioTrack(index=2, language="fra", codec="ac3"),
    ]

    # Find English track
    track = get_audio_track_by_language(tracks, "eng")
    assert track is not None
    assert track.index == 0
    assert track.language == "eng"

    # Find Spanish track
    track = get_audio_track_by_language(tracks, "spa")
    assert track is not None
    assert track.index == 1

    # Language not found
    track = get_audio_track_by_language(tracks, "jpn")
    assert track is None

    # Empty list
    track = get_audio_track_by_language([], "eng")
    assert track is None


def test_get_audio_track_by_language_case_insensitive():
    """Test language matching is case-insensitive."""
    tracks = [
        AudioTrack(index=0, language="ENG", codec="aac"),
    ]

    track = get_audio_track_by_language(tracks, "eng")
    assert track is not None
    assert track.index == 0
