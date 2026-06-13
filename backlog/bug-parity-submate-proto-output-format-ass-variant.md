# bug-parity: submate-proto `OutputFormat` has an extra `Ass` variant not in the Python spec

## Falsifier

A new parity test, modeled on `submate-queue/tests/parity.rs::queue_enum_values`,
pins `submate_proto::OutputFormat` against the golden `rust/fixtures/queue/enum_values.json`:

```
cargo test --manifest-path rust/Cargo.toml -p submate-proto --test parity proto_output_format_matches_golden
```

This test does NOT exist today (submate-proto has only inline `mod tests`, no
`tests/parity.rs`, and no `parity` dev-dependency). When written, it must
assert that the set of `submate_proto::OutputFormat` variants and their
serde wire strings is EXACTLY the golden's `OutputFormat` table:

```json
"OutputFormat": { "JSON": "json", "SRT": "srt", "TXT": "txt", "VTT": "vtt" }
```

The test FAILS as currently implemented because the Rust enum carries a fifth
variant (`Ass => "ass"`) that has no entry in the golden / Python spec.

## The divergence (exact)

Python SPEC — `submate/queue/models.py`, `class OutputFormat(Enum)` — has
exactly four members:

| name | .value |
|------|--------|
| SRT  | srt    |
| VTT  | vtt    |
| TXT  | txt    |
| JSON | json   |

There is no `ASS`. The bazarr dispatch (`submate/queue/services/bazarr.py`)
`match output_format` has arms only for SRT/VTT/TXT/JSON; the `_` arm raises
`ValueError`. `OutputFormat.from_value("ass")` does `cls("ass")` which raises
`ValueError`, caught and coerced to the default `OutputFormat.SRT`.

Rust — `rust/crates/submate-proto/src/lib.rs`, `pub enum OutputFormat` (added
this round) — has FIVE variants:

```rust
#[serde(rename_all = "lowercase")]
pub enum OutputFormat { Srt(default), Vtt, Ass, Json, Txt }
```

with `extension()` mapping `Ass => ".ass"`. The same 5-variant set is mirrored
in `rust/crates/submate-cli/src/main.rs` (`enum OutputFormat { ... Ass, ... }`,
exposed as a clap `--format`/`-F` value-enum) and consumed by `to_ass` in
`rust/crates/stable-ts/src/output.rs`.

### Golden value vs Rust value (first divergence)

- Golden `OutputFormat` key set: `{JSON, SRT, TXT, VTT}` (4 keys).
- Rust `submate_proto::OutputFormat` variant set: `{Srt, Vtt, Ass, Json, Txt}` (5).
- Extra key in Rust, absent in golden: **`Ass` / `"ass"`**.

## User-visible impact

`output_format: "ass"` is a reachable wire/CLI value in Rust:

- `submate-cli`: `submate transcribe -F ass <file>` parses to `OutputFormat::Ass`
  and writes a `.ass` file. The Python `transcribe` CLI has no `--format` option
  at all and only ever emits `.srt`/`.vtt` via `result.to_srt_vtt()`.
- A `JobOpts` JSON payload `{"output_format":"ass", ...}` deserializes
  successfully in Rust (`OutputFormat::Ass`), whereas Python's
  `OutputFormat.from_value("ass")` silently falls back to `SRT`. Same input,
  different format emitted — a byte-for-byte enum-value contract violation
  (`enum .value strings ... must match Python byte-for-byte`).

## Two `OutputFormat` enums, only one pinned

`submate-queue::OutputFormat` IS correct (4 variants) and is already
parity-pinned by `submate-queue/tests/parity.rs::queue_enum_values` +
`no_uncovered_enums_in_golden` against the same golden. The new
`submate-proto::OutputFormat` (and the `submate-cli` mirror) is a SECOND,
divergent definition with no parity coverage. The fix should make the proto
enum's variant/wire-string set identical to the golden and to
`submate-queue::OutputFormat`.

## Suggested fix direction (for the implementer — do not assume)

Either (a) drop the `Ass` variant from `submate_proto::OutputFormat` and the
`submate-cli` mirror so the public surface matches Python's 4-format contract
(stable-ts `to_ass` can stay as an upstream-library port, just not reachable
via `OutputFormat`), or (b) if ASS is an intentional Rust-only superset,
document it as an explicit, sanctioned divergence in `rust/fixtures/README.md`
and exclude it from the enum-value parity contract. Option (a) preserves the
byte-for-byte enum contract; option (b) requires a contract amendment.

## Provenance

- Spec: `submate/queue/models.py` (`OutputFormat`), `submate/queue/services/bazarr.py` (dispatch arms).
- Golden: `rust/fixtures/queue/enum_values.json`.
- Rust: `rust/crates/submate-proto/src/lib.rs` (`OutputFormat`, added this round),
  `rust/crates/submate-cli/src/main.rs` (mirror + clap surface).
- Existing correct precedent: `rust/crates/submate-queue/tests/parity.rs`.
