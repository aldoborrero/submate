"""Capture the golden for the queue result-envelope wire contract.

These are the five canonical JSON envelopes the registered Huey tasks emit
across the worker -> server boundary (see
`submate/queue/registered_tasks.py`):

    transcribe success: {"success": true, "data": <subtitle|TranscriptionResult>}
    transcribe skip:    {"success": true, "skipped": true, "reason": "<skip>", "data": null}
    transcribe failure: {"success": false, "error": "<str>", "data": null}
    detect success:     {"success": true, "data": {"detected_language","language_code"}}
    detect failure:     {"success": false, "error": "<str>",
                         "data": {"detected_language":"Unknown","language_code":"und"}}

The task *bodies* need whisper/config (external runtime), so we do NOT call
them. Instead we build the same envelopes from the real model classes
(`TaskResult`, `TranscriptionResult`, `TranscriptionSkippedError`,
`SkipReason`) so every field name and the `reason.value` string stay
Python-sourced and cannot drift from the spec. The literal dict shapes mirror
the exact `return {...}` statements in `registered_tasks.py`.

This script is NOT part of the grind. A human runs it once (and again if the
Python envelopes change), then commits:

    rust/fixtures/queue/task_envelopes.json
"""

from __future__ import annotations

from dataclasses import asdict

from submate.queue.models import (
    SkipReason,
    TranscriptionResult,
    TranscriptionSkippedError,
)

from _common import write_json


def main() -> None:
    # A representative TranscriptionResult, serialized as the worker would for
    # the data-rich success path (transcribe_file_task returns TaskResult.data).
    transcription = TranscriptionResult(
        subtitle_path="/data/movie.en.srt",
        language="en",
        segments=42,
        text="Hello, world.",
    )

    # The skip envelope's reason string comes from the real enum value, and the
    # error message default mirrors TranscriptionSkippedError(message or value).
    skip = TranscriptionSkippedError(SkipReason.TARGET_SUBTITLE_EXISTS)

    envelopes = {
        # transcribe_audio_task / transcribe_file_task success
        "transcribe_success_subtitle": {
            "success": True,
            "data": "1\n00:00:00,000 --> 00:00:01,000\nHello, world.\n",
        },
        "transcribe_success_result": {
            "success": True,
            "data": asdict(transcription),
        },
        # transcribe_file_task skip (pinned by tests/test_queue.py)
        "transcribe_skip": {
            "success": True,
            "skipped": True,
            "reason": skip.reason.value,
            "data": None,
        },
        # transcribe_*_task failure
        "transcribe_failure": {
            "success": False,
            "error": "boom",
            "data": None,
        },
        # detect_language_task success
        "detect_success": {
            "success": True,
            "data": {"detected_language": "English", "language_code": "en"},
        },
        # detect_language_task failure (the und/Unknown default)
        "detect_failure": {
            "success": False,
            "error": "boom",
            "data": {"detected_language": "Unknown", "language_code": "und"},
        },
    }

    write_json("queue/task_envelopes.json", envelopes)


if __name__ == "__main__":
    main()
