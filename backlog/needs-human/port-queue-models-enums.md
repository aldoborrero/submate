# Port queue/models.py enums: OutputFormat + SkipReason

**blocked-by:** _(none — foundational pure-data, no unported deps)_

## what
Port the two pure-data enums in `submate/queue/models.py` that the
existing `rust/fixtures/types/enum_values.json` golden does **not** cover
(it only captures the six `submate/types.py` enums):

- **`OutputFormat`** (`Enum`): members `SRT="srt"`, `VTT="vtt"`, `TXT="txt"`,
  `JSON="json"`. Plus two behaviors that are load-bearing:
  - `.extension` -> `f".{value}"` (e.g. `".srt"`).
  - `.from_value(value, default=None)` coercion: returns an existing
    `OutputFormat` unchanged; coerces a known string; for an **unknown**
    string returns `default` if given else `OutputFormat.SRT` (never raises).
    Exact semantics are pinned by `tests/test_queue.py::test_output_format_from_value_normalizes`:
    `from_value("vtt") is VTT`, `from_value(JSON) is JSON`,
    `from_value("nonsense") is SRT`, `from_value("nonsense", default=TXT) is TXT`.
- **`SkipReason`** (`StrEnum`, 11 members): `NOT_SKIPPED="not_skipped"`,
  `TARGET_SUBTITLE_EXISTS="target_subtitle_exists"`,
  `EXTERNAL_SUBTITLE_EXISTS="external_subtitle_exists"`,
  `INTERNAL_SUBTITLE_LANGUAGE_EXISTS="internal_subtitle_language_exists"`,
  `SUBTITLE_LANGUAGE_IN_SKIP_LIST="subtitle_language_in_skip_list"`,
  `AUDIO_LANGUAGE_IN_SKIP_LIST="audio_language_in_skip_list"`,
  `UNKNOWN_LANGUAGE="unknown_language"`,
  `NO_PREFERRED_AUDIO_LANGUAGE="no_preferred_audio_language"`,
  `LRC_FILE_EXISTS="lrc_file_exists"`,
  `LANGUAGE_NOT_SET_BUT_SUBTITLES_EXIST="language_not_set_but_subtitles_exist"`.
  As a `StrEnum` the `.value` strings are also the on-the-wire `reason`
  field returned by the worker task envelope (`tests/test_queue.py` asserts
  `result["reason"] == SkipReason.TARGET_SUBTITLE_EXISTS.value`), so they
  must be **byte-for-byte** exact.

## where
`rust/crates/submate-queue/src/models.rs` (new module; re-export from the
crate root). Derive `serde` Serialize/Deserialize with explicit
`#[serde(rename = "...")]` per variant so a naive derive cannot mangle
`not_skipped` -> `NotSkipped`, etc. `from_value` is a `fn(Option<&str>,
default) -> OutputFormat` helper (NOT `FromStr`, because unknown strings
must fall back, never error).

## why
These enums are imported by `queue/tasks/{bazarr,transcription}.py`,
`queue/services/{bazarr,transcription}.py`, `registered_tasks.py`, and the
Bazarr server boundary. They are the result-routing vocabulary the whole
server↔node system speaks; every downstream queue/server item depends on
these strings matching Python. Cheap, dependency-free, high-leverage — lands
before the queue store and services.

## falsifies
`cargo test -p submate-queue parity::queue_enum_values` asserts **exact**
(`parity::assert_json_eq`) equality between a serialized `{VARIANT -> value}`
map for `OutputFormat` and `SkipReason` and the golden, PLUS a unit test
`from_value_coercion` reproducing the four `test_output_format_from_value_normalizes`
cases and `.extension == ".srt"` for SRT.

**requires fixture: `rust/fixtures/types/enum_values.json` must be extended
to include `OutputFormat` and `SkipReason` (capture first).** The golden is
merge-denylisted for the porter — a human/capture run must add these two
enums to `rust/fixtures/capture/capture_enums.py`'s `ENUMS` list (import
`OutputFormat`/`SkipReason` from `submate.queue.models`) and re-run capture.
Until then this falsifier cannot pass; flag for capture.
