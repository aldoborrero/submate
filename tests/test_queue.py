"""Tests for queue tasks - transcription and Bazarr."""

from pathlib import Path
from unittest.mock import Mock, patch

import pytest

from submate.queue.models import OutputFormat, TranscriptionResult
from submate.queue.tasks.bazarr import BazarrTranscriptionTask, LanguageDetectionTask
from submate.queue.tasks.transcription import TranscriptionTask

# Transcription task tests


def test_transcription_validate_input_valid():
    """Test validation passes for existing file."""
    task = TranscriptionTask(Mock(), transcription_service=Mock())

    with patch("pathlib.Path.exists", return_value=True):
        task.validate_input(file_path="/valid.mp4")


def test_transcription_validate_input_invalid():
    """Test validation fails for non-existent file."""
    task = TranscriptionTask(Mock(), transcription_service=Mock())

    with pytest.raises(ValueError, match="File does not exist"):
        task.validate_input(file_path="/nonexistent.mp4")


def test_transcription_execute_success():
    """Test successful transcription execution."""
    service = Mock()
    task = TranscriptionTask(Mock(), transcription_service=service)

    mock_result = TranscriptionResult(subtitle_path="/path/to/sub.srt", language="en", segments=5, text="Test")
    service.transcribe_file.return_value = mock_result

    result = task.execute(file_path="/input.mp4", audio_language="en", translate_to=None, force=False)

    assert result.success is True
    assert result.data == mock_result
    service.transcribe_file.assert_called_once_with(Path("/input.mp4"), "en", None, False)


def test_transcription_execute_failure():
    """Test transcription failure returns error result."""
    service = Mock()
    task = TranscriptionTask(Mock(), transcription_service=service)

    service.transcribe_file.side_effect = Exception("Transcription failed")

    result = task.execute(file_path="/input.mp4")

    assert result.success is False
    assert result.error == "Transcription failed"


# Bazarr task tests


def test_bazarr_transcription_execute():
    """Test Bazarr transcription task execution."""
    service = Mock()
    task = BazarrTranscriptionTask(Mock(), bazarr_service=service)

    service.transcribe_audio_bytes.return_value = "subtitle content"

    result = task.execute(
        audio_bytes=b"test_audio",
        language="en",
        task="transcribe",
        output_format=OutputFormat.SRT,
        word_timestamps=True,
        target_language=None,
    )

    assert result.success is True
    assert result.data == "subtitle content"
    service.transcribe_audio_bytes.assert_called_once_with(
        audio_bytes=b"test_audio",
        language="en",
        task="transcribe",
        output_format=OutputFormat.SRT,
        word_timestamps=True,
        target_language=None,
    )


def test_bazarr_transcription_with_translation():
    """Test Bazarr transcription with target language for translation."""
    service = Mock()
    task = BazarrTranscriptionTask(Mock(), bazarr_service=service)

    # Service should return translated content
    service.transcribe_audio_bytes.return_value = "contenido de subtítulos traducido"

    result = task.execute(
        audio_bytes=b"test_audio",
        language=None,  # Auto-detect source
        task="transcribe",
        output_format=OutputFormat.SRT,
        word_timestamps=False,
        target_language="es",  # Translate to Spanish
    )

    assert result.success is True
    assert result.data == "contenido de subtítulos traducido"
    service.transcribe_audio_bytes.assert_called_once_with(
        audio_bytes=b"test_audio",
        language=None,
        task="transcribe",
        output_format=OutputFormat.SRT,
        word_timestamps=False,
        target_language="es",
    )


def test_language_detection_execute():
    """Test language detection task execution."""
    service = Mock()
    task = LanguageDetectionTask(Mock(), bazarr_service=service)

    expected = {"detected_language": "English", "language_code": "en"}
    service.detect_language.return_value = expected

    result = task.execute(audio_bytes=b"test_audio")

    assert result.success is True
    assert result.data == expected
    service.detect_language.assert_called_once_with(b"test_audio")


# Queued-dispatch tests: non-immediate enqueue must route through a statically
# registered Huey task so a separate worker process can find and execute it.


def test_registered_task_for_transcription():
    """TranscriptionTask maps to the statically registered transcribe_file_task."""
    import submate.queue.registered_tasks as registered_tasks
    from submate.queue.task_queue import TaskQueue

    queue = TaskQueue.__new__(TaskQueue)  # skip service/huey initialization
    task = TranscriptionTask(Mock(), transcription_service=Mock())

    assert queue._registered_task_for(task) is registered_tasks.transcribe_file_task


def test_registered_task_for_unknown_raises():
    """A task with no registered worker task cannot be queued."""
    from submate.queue.task_queue import TaskQueue

    queue = TaskQueue.__new__(TaskQueue)
    task = LanguageDetectionTask(Mock(), bazarr_service=Mock())

    with pytest.raises(ValueError, match="No statically-registered"):
        queue._registered_task_for(task)


def test_transcribe_file_task_handles_skip():
    """A skip is swallowed (no exception) so Huey doesn't retry it. The task is
    fire-and-forget, so it returns None and stores no result."""
    from submate.queue.models import SkipReason, TranscriptionSkippedError

    service = Mock()
    service.transcribe_file.side_effect = TranscriptionSkippedError(SkipReason.TARGET_SUBTITLE_EXISTS)

    with (
        patch("submate.queue.services.TranscriptionService", return_value=service),
        patch("submate.config.get_config", return_value=Mock()),
    ):
        from submate.queue.registered_tasks import transcribe_file_task

        result = transcribe_file_task.call_local(file_path="/movie.mkv")

    assert result is None
    service.transcribe_file.assert_called_once()


def test_transcribe_file_task_drops_duplicate_inflight():
    """A task whose params are already in-flight is dropped without re-running."""
    import submate.queue.registered_tasks as rt

    service = Mock()
    key = ("/movie.mkv", None, None, False)
    rt._inflight_tasks.add(key)
    try:
        with (
            patch("submate.queue.services.TranscriptionService", return_value=service),
            patch("submate.config.get_config", return_value=Mock()),
        ):
            result = rt.transcribe_file_task.call_local(file_path="/movie.mkv")

        assert result is None
        service.transcribe_file.assert_not_called()
    finally:
        rt._inflight_tasks.discard(key)


def test_transcribe_file_task_reraises_failure_for_retry():
    """A genuine failure propagates so Huey's retry machinery engages, and the
    in-flight marker is still cleared (so the retry can run)."""
    import submate.queue.registered_tasks as rt

    service = Mock()
    service.transcribe_file.side_effect = RuntimeError("disk error")

    with (
        patch("submate.queue.services.TranscriptionService", return_value=service),
        patch("submate.config.get_config", return_value=Mock()),
    ):
        with pytest.raises(RuntimeError, match="disk error"):
            rt.transcribe_file_task.call_local(file_path="/movie.mkv")

    assert ("/movie.mkv", None, None, False) not in rt._inflight_tasks


def test_transcribe_file_task_releases_inflight_after_run():
    """The in-flight marker is cleared once the task finishes, so a later
    (sequential) re-enqueue of the same file is allowed to run."""
    import submate.queue.registered_tasks as rt

    service = Mock()
    service.transcribe_file.return_value = TranscriptionResult(
        subtitle_path="/out.srt", language="en", segments=1, text="hi"
    )
    with (
        patch("submate.queue.services.TranscriptionService", return_value=service),
        patch("submate.config.get_config", return_value=Mock()),
    ):
        rt.transcribe_file_task.call_local(file_path="/movie.mkv")

    assert ("/movie.mkv", None, None, False) not in rt._inflight_tasks


def test_output_format_from_value_normalizes():
    from submate.queue.models import OutputFormat

    assert OutputFormat.from_value("vtt") is OutputFormat.VTT
    assert OutputFormat.from_value(OutputFormat.JSON) is OutputFormat.JSON
    assert OutputFormat.from_value("nonsense") is OutputFormat.SRT
    assert OutputFormat.from_value("nonsense", default=OutputFormat.TXT) is OutputFormat.TXT


def test_queued_transcription_runs_in_a_separate_worker_registry(tmp_path, monkeypatch):
    """End-to-end: a task enqueued by TaskQueue must be dequeued and executed by a
    worker whose registry was populated independently (the original bug was that
    a per-call dynamic task name existed only in the enqueuing process)."""
    from huey import SqliteHuey

    import submate.queue.registered_tasks as registered_tasks
    import submate.queue.task_queue as tq_mod
    from submate.queue.models import TranscriptionResult
    from submate.queue.task_queue import TaskQueue
    from submate.queue.tasks import TranscriptionTask

    db = str(tmp_path / "queue.db")
    producer = SqliteHuey("submate", filename=db, results=True, utc=True)
    worker = SqliteHuey("submate", filename=db, results=True, utc=True)

    # Register the production task function on both registries: the producer (the
    # process calling enqueue) and a separate worker process that imported
    # registered_tasks. They share the SQLite broker but have distinct registries.
    func = registered_tasks.transcribe_file_task.func
    producer_task = producer.task(name="transcribe_file_task")(func)
    worker.task(name="transcribe_file_task")(func)
    monkeypatch.setattr(registered_tasks, "transcribe_file_task", producer_task)
    monkeypatch.setattr(tq_mod, "get_huey", lambda: producer)

    # Keep execution in-process: stub the heavy service and config.
    fake_result = TranscriptionResult(subtitle_path="/out.srt", language="en", segments=1, text="hi")
    service = Mock()
    service.transcribe_file.return_value = fake_result
    monkeypatch.setattr("submate.queue.services.TranscriptionService", lambda config: service)
    monkeypatch.setattr("submate.config.get_config", lambda: Mock())

    video = tmp_path / "movie.mkv"
    video.write_text("x")

    queue = TaskQueue.__new__(TaskQueue)
    queue.config = Mock()
    queue.transcription_service = Mock()
    queue.bazarr_service = Mock()

    try:
        # Producer enqueues (non-immediate) -> writes a message to the shared broker.
        queue.enqueue(TranscriptionTask, file_path=str(video), audio_language=None, translate_to=None, force=False)

        # Worker dequeues (deserializes via its own registry) and executes.
        task = worker.dequeue()
        assert task is not None
        result = worker.execute(task)

        # Fire-and-forget: the worker runs the service but stores no result.
        assert result is None
        assert worker.storage.result_store_size() == 0
        service.transcribe_file.assert_called_once_with(video, None, None, False)
    finally:
        producer.storage.close()
        worker.storage.close()
