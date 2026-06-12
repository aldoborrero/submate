# Port queue/models.py result envelopes + skip error

**blocked-by:** port-queue-models-enums

## what
Port the result-envelope data types in `submate/queue/models.py` that cross
the worker -> server JSON boundary (every registered task returns one of
these dicts; see `registered_tasks.py` and `tests/test_queue.py`):

- **`TaskResult[T]`** (dataclass): `success: bool`, `data: T | None = None`,
  `error: str | None = None`, `metadata: dict | None = None`.
- **`TranscriptionResult`** (dataclass): `subtitle_path: str`,
  `language: str`, `segments: int`, `text: str`.
- **`LanguageDetectionResult`** (`TypedDict`): `detected_language: str`,
  `language_code: str` — and its failure default
  `{"detected_language": "Unknown", "language_code": "und"}` returned by
  `detect_language_task` on error.
- **`TranscriptionSkippedError`** (exception carrying a `SkipReason`):
  `message` defaults to `reason.value`. The worker turns this into the
  envelope `{"success": True, "skipped": True, "reason": <reason.value>,
  "data": None}` (pinned by `tests/test_queue.py`, which asserts
  `result["reason"] == SkipReason.TARGET_SUBTITLE_EXISTS.value`).

Match the exact **JSON envelope shapes** the registered tasks emit, NOT a
new Rust-idiomatic shape, because Bazarr/server handlers and node clients
deserialize these by field name:
- transcribe success: `{"success": true, "data": <subtitle string|TranscriptionResult>}`
- transcribe skip:    `{"success": true, "skipped": true, "reason": "<skip_reason>", "data": null}`
- transcribe failure: `{"success": false, "error": "<str>", "data": null}`
- detect success:     `{"success": true, "data": {"detected_language": "...", "language_code": "..."}}`
- detect failure:     `{"success": false, "error": "<str>", "data": {"detected_language": "Unknown", "language_code": "und"}}`

## where
`rust/crates/submate-queue/src/models.rs` alongside the enums. `serde`
Serialize/Deserialize; keep `skipped`/`reason` as optional fields so the
three transcribe-envelope variants round-trip to the same struct. Reuse the
`SkipReason` from `port-queue-models-enums`.

## why
This is the wire contract between a processing node and the server. The
field names and the skip/error envelope shapes are what the Bazarr ASR
handler, the node API, and the result-write path all depend on; a drift
here silently breaks every integration even with correct transcription.

## falsifies
`cargo test -p submate-queue parity::task_envelopes` asserts **exact**
(`parity::assert_json_eq`) round-trip of each envelope variant (transcribe
success/skip/failure, detect success/failure) against the golden, including
the `und`/`Unknown` detect-failure default and `reason` == the `SkipReason`
`.value` string.

**requires fixture: `rust/fixtures/queue/task_envelopes.json` (capture
first).** No `rust/fixtures/queue/` dir exists yet and the porter cannot
write goldens (denylisted). A capture script (e.g.
`rust/fixtures/capture/capture_queue.py`) must emit the five canonical
envelope JSON objects above — driving the real `registered_tasks` code
paths where feasible, else constructing them from `TaskResult` /
`TranscriptionSkippedError` so the shapes stay Python-sourced. Flag for
capture; falsifier blocked until the golden lands.
