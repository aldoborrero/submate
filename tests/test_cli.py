"""Tests for CLI commands."""

from unittest.mock import MagicMock, Mock, patch

from click.testing import CliRunner

from submate.cli import cli


def test_cli_help():
    """Test that CLI shows help."""
    runner = CliRunner()
    result = runner.invoke(cli, ["--help"])
    assert result.exit_code == 0
    assert "submate" in result.output.lower()
    assert "transcribe" in result.output.lower()


def test_cli_version():
    """Test version flag."""
    runner = CliRunner()
    result = runner.invoke(cli, ["--version"])
    assert result.exit_code == 0
    assert "version" in result.output.lower()


def test_config_show(monkeypatch):
    """Test config show command."""
    monkeypatch.setenv("SUBMATE__WHISPER__MODEL", "large")
    monkeypatch.setenv("SUBMATE__WHISPER__DEVICE", "cuda")

    runner = CliRunner()
    result = runner.invoke(cli, ["config", "show"])

    assert result.exit_code == 0
    assert "large" in result.output
    assert "cuda" in result.output


def test_config_show_validates():
    """Test config show validates configuration."""
    runner = CliRunner()
    result = runner.invoke(cli, ["config", "show"])

    assert result.exit_code == 0
    # Should show defaults even without env vars
    assert "whisper" in result.output.lower()


def test_transcribe_help():
    """Test transcribe command help."""
    runner = CliRunner()
    result = runner.invoke(cli, ["transcribe", "--help"])

    assert result.exit_code == 0
    assert "--audio-language" in result.output
    assert "--force" in result.output
    assert "--log-level" in result.output


def test_transcribe_file_not_found():
    """Test transcribe with non-existent file."""
    runner = CliRunner()
    result = runner.invoke(cli, ["transcribe", "nonexistent.mp4"])

    assert result.exit_code != 0
    assert "not found" in result.output.lower() or "does not exist" in result.output.lower()


@patch("submate.cli.commands.transcribe.get_task_queue")
def test_transcribe_single_file(mock_get_queue, tmp_path):
    """Test transcribing a single file."""
    # Setup
    video_file = tmp_path / "test.mp4"
    video_file.write_bytes(b"fake video")

    # Mock TaskQueue
    mock_queue = MagicMock()
    mock_queue.enqueue.return_value = MagicMock()
    mock_queue.size = 0
    mock_get_queue.return_value = mock_queue

    # Run
    runner = CliRunner()
    result = runner.invoke(cli, ["transcribe", str(video_file), "--sync"])

    # Verify
    assert result.exit_code == 0
    mock_queue.enqueue.assert_called_once()
    # Check that enqueue was called with correct arguments
    call_args = mock_queue.enqueue.call_args
    assert str(video_file) in str(call_args)


@patch("submate.cli.commands.transcribe.JellyfinClient")
@patch("submate.cli.commands.transcribe.get_task_queue")
def test_transcribe_with_jellyfin_refresh(mock_get_queue, mock_jellyfin_class, tmp_path):
    """Test transcribe with Jellyfin library refresh."""
    video_file = tmp_path / "test.mp4"
    video_file.write_bytes(b"fake video")

    # Mock TaskQueue
    mock_queue = MagicMock()
    mock_queue.enqueue.return_value = MagicMock()
    mock_queue.size = 0
    mock_get_queue.return_value = mock_queue

    # Mock Jellyfin client
    mock_jellyfin = Mock()
    mock_jellyfin.is_configured.return_value = True
    mock_jellyfin.refresh_all_libraries.return_value = ["Movies", "TV Shows"]
    mock_jellyfin_class.return_value = mock_jellyfin

    runner = CliRunner()
    result = runner.invoke(cli, ["transcribe", str(video_file), "--sync", "--refresh-jellyfin"])

    assert result.exit_code == 0
    mock_jellyfin.connect.assert_called_once()
    mock_jellyfin.refresh_all_libraries.assert_called_once()


def test_server_help():
    """Test server command help."""
    runner = CliRunner()
    result = runner.invoke(cli, ["server", "--help"])

    assert result.exit_code == 0
    assert "webhook" in result.output.lower() or "server" in result.output.lower()


class TestTranslateCommandASS:
    """Tests for translate command ASS support."""

    def test_translate_recognizes_ass_files(self, tmp_path):
        """Verify translate command accepts .ass files."""
        from submate.cli.commands.translate import SUBTITLE_EXTENSIONS, is_subtitle_file

        assert ".ass" in SUBTITLE_EXTENSIONS
        assert ".ssa" in SUBTITLE_EXTENSIONS

        ass_file = tmp_path / "test.ass"
        ass_file.write_text("[Script Info]\nTitle: Test")

        from pathlib import Path

        assert is_subtitle_file(Path(ass_file))
