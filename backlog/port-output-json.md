# port-output: JSON and plain-text output

## what
Add two more output serializers alongside SRT/VTT/ASS:

- **JSON** — the full transcription result, matching Python's
  `OutputFormat.JSON` path in `submate/queue/services/bazarr.py`:
  `json.dumps(result.to_dict())`. The Rust `WhisperResult::to_dict()` already
  exists (model.rs) and is value-parity-tested (`parity::model_roundtrip`); this
  item just serializes that `to_dict()` Value to a compact single-line JSON
  string and exposes it as output.
- **TXT** — plain text, matching `OutputFormat.TXT`: the result's full `.text`
  (the concatenated transcript, no timestamps).

JSON uses **value-parity** (not byte-parity): Python's `json.dumps` key ordering,
separator spacing (`", "`/`": "`), and `ensure_ascii` escaping are formatting
details a JSON consumer does not care about, and matching them byte-for-byte in
Rust is brittle. Assert the emitted JSON *parses back to the same Value* as the
golden instead. (If a future item needs the exact bazarr API bytes, it can pin
formatting separately.)

## where
- `rust/crates/stable-ts/src/model.rs` (next to `to_dict`) or `output.rs` —
  `pub fn to_json(result: &WhisperResult) -> String` (serialize `to_dict()`),
  and `pub fn to_txt(result: &WhisperResult) -> String` (the result text).
- `rust/crates/submate-whisper/src/lib.rs` — `to_json(&self)` / `to_txt(&self)`
  wrappers, mirroring `to_srt_vtt`.

## why
JSON is the richest export (carries word-level timings + metadata, the basis for
any downstream tooling) and is already part of submate's Python `OutputFormat`;
TXT is the trivial transcript export. Both are missing from the Rust port, which
emits only SRT/VTT.

## falsifies
`cargo test -p stable-ts` green, including:
- `parity::output_json`: parse `stablets/clipA/00_raw.json`, and assert
  `serde_json::from_str(&to_json(&result))` equals the golden Value parsed from
  `rust/fixtures/stablets/clipA/output.json` (== the result's `to_dict()`). Proves
  the JSON output round-trips the full result faithfully.
- `parity::output_txt`: assert `to_txt(&result)` equals the result's `text`
  field from `00_raw.json` (the `text` value verbatim).
