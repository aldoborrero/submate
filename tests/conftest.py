"""Shared pytest fixtures for submate tests."""

from pathlib import Path

import pytest


@pytest.fixture
def temp_dir(tmp_path: Path) -> Path:
    """Provide a temporary directory for tests."""
    return tmp_path


@pytest.fixture
def sample_video_path(temp_dir: Path) -> Path:
    """Provide a path to a mock video file."""
    video = temp_dir / "sample.mp4"
    video.touch()
    return video


@pytest.fixture
def sample_subtitle_content() -> str:
    """Provide sample SRT subtitle content."""
    return """1
00:00:00,000 --> 00:00:02,000
Hello, world!

2
00:00:02,000 --> 00:00:05,000
This is a test subtitle.
"""
