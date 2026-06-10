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


def test_enqueue_files_fail_fast_stops_on_first_error(monkeypatch):
    """--fail-fast must stop processing after the first failure."""
    import importlib
    from pathlib import Path

    transcribe_mod = importlib.import_module("submate.cli.commands.transcribe")

    task_queue = Mock()
    task_queue.enqueue.side_effect = RuntimeError("boom")
    task_queue.size = 0
    monkeypatch.setattr(transcribe_mod, "get_task_queue", lambda: task_queue)

    files = [Path("/a.mkv"), Path("/b.mkv"), Path("/c.mkv")]
    transcribe_mod._enqueue_files(files, None, None, False, immediate=False, fail_fast=True)

    assert task_queue.enqueue.call_count == 1


def test_enqueue_files_without_fail_fast_continues(monkeypatch):
    """Default behavior keeps processing all files despite errors."""
    import importlib
    from pathlib import Path

    transcribe_mod = importlib.import_module("submate.cli.commands.transcribe")

    task_queue = Mock()
    task_queue.enqueue.side_effect = RuntimeError("boom")
    task_queue.size = 0
    monkeypatch.setattr(transcribe_mod, "get_task_queue", lambda: task_queue)

    files = [Path("/a.mkv"), Path("/b.mkv"), Path("/c.mkv")]
    transcribe_mod._enqueue_files(files, None, None, False, immediate=False, fail_fast=False)

    assert task_queue.enqueue.call_count == 3


def test_config_show_flattens_nested_settings(monkeypatch):
    """Nested settings must render as readable rows, not raw dict reprs."""
    monkeypatch.setenv("SUBMATE__WHISPER__MODEL", "large")

    runner = CliRunner()
    result = runner.invoke(cli, ["config", "show"])

    assert result.exit_code == 0
    assert "large" in result.output
    assert "{'" not in result.output  # no raw Python dict repr leaking through


def test_translate_output_with_multiple_files_errors(tmp_path):
    """--output with more than one input file must error, not be silently ignored."""
    (tmp_path / "a.srt").write_text("1\n00:00:01,000 --> 00:00:02,000\nHi\n\n")
    (tmp_path / "b.srt").write_text("1\n00:00:01,000 --> 00:00:02,000\nHi\n\n")

    runner = CliRunner()
    result = runner.invoke(cli, ["translate", str(tmp_path), "-t", "es", "-o", str(tmp_path / "out.srt")])

    assert result.exit_code != 0
    assert "single input file" in result.output.lower()


def test_detect_source_language_uses_valid_tag():
    from pathlib import Path

    from submate.cli.commands.translate import detect_source_language

    assert detect_source_language(Path("movie.fr.srt"), "auto") == "fr"


def test_detect_source_language_rejects_non_language_token():
    from pathlib import Path

    from submate.cli.commands.translate import detect_source_language

    assert detect_source_language(Path("movie.v2.srt"), "auto") == "en"
    assert detect_source_language(Path("episode.01.srt"), "auto") == "en"


def test_detect_source_language_respects_explicit():
    from pathlib import Path

    from submate.cli.commands.translate import detect_source_language

    assert detect_source_language(Path("movie.fr.srt"), "es") == "es"
