"""Tests for subtitle filename language parsing."""

from pathlib import Path

from submate.language import LanguageCode
from submate.subtitle import get_external_subtitle_paths, parse_subtitle_language


def test_parse_simple_language_tag():
    assert parse_subtitle_language(Path("movie.en.srt"), "movie") == LanguageCode.ENGLISH


def test_parse_three_letter_code():
    assert parse_subtitle_language(Path("movie.eng.srt"), "movie") == LanguageCode.ENGLISH


def test_parse_subgen_pattern():
    """Docstring example: language tag is the last segment."""
    assert parse_subtitle_language(Path("movie.subgen.medium.en.srt"), "movie") == LanguageCode.ENGLISH


def test_parse_prefers_trailing_tag_over_earlier_collision():
    """A non-language flag whose token collides with an ISO code ('no' -> Norwegian,
    'forced') precedes the real language tag; the trailing tag must win."""
    assert parse_subtitle_language(Path("movie.no.forced.en.srt"), "movie") == LanguageCode.ENGLISH


def test_parse_trailing_flag_after_language():
    """A trailing non-language flag should not hide the real language tag."""
    assert parse_subtitle_language(Path("movie.en.forced.srt"), "movie") == LanguageCode.ENGLISH


def test_parse_no_language_returns_none():
    assert parse_subtitle_language(Path("movie.srt"), "movie") == LanguageCode.NONE


def test_parse_unrelated_stem_returns_none():
    assert parse_subtitle_language(Path("other.en.srt"), "movie") == LanguageCode.NONE


def test_parse_requires_dot_boundary():
    """'Episode 10.en' must not be treated as a subtitle for video 'Episode 1'."""
    assert parse_subtitle_language(Path("Episode 10.en.srt"), "Episode 1") == LanguageCode.NONE


def test_external_paths_match_exact_and_dot_boundary(tmp_path):
    video = tmp_path / "Episode 1.mkv"
    video.write_text("x")
    (tmp_path / "Episode 1.srt").write_text("x")  # exact stem match
    (tmp_path / "Episode 1.en.srt").write_text("x")  # dot-boundary match

    names = {p.name for p in get_external_subtitle_paths(video)}

    assert "Episode 1.srt" in names
    assert "Episode 1.en.srt" in names


def test_external_paths_reject_prefix_collision(tmp_path):
    """A sibling whose stem merely starts with the video stem (no dot boundary)
    must not be matched."""
    video = tmp_path / "Episode 1.mkv"
    video.write_text("x")
    (tmp_path / "Episode 10.en.srt").write_text("x")

    names = {p.name for p in get_external_subtitle_paths(video)}

    assert "Episode 10.en.srt" not in names
