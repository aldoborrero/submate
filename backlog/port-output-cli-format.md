# port-output: `--format` flag to choose the output format

**blocked-by:** port-output-ass, port-output-json

## what
Let the user pick the output format instead of always writing SRT. Add
`-F`/`--format <srt|vtt|ass|json|txt>` to `submate transcribe` (default `srt`,
preserving today's behavior), plumb the choice through to the node that
assembles the result, and write the output file with the matching extension
(`.srt`/`.vtt`/`.ass`/`.json`/`.txt`).

Today the node hardcodes SRT: `assembled.to_srt_vtt(false)` in the
`whisper_processor` (`submate-node`), and the CLI writes `file.with_extension("srt")`
in the `--sync` result handler. Both must honor the requested format:
- `srt` → `to_srt_vtt(false, false)`  · `vtt` → `to_srt_vtt(false, true)`
- `ass` → `to_ass(false)` (from port-output-ass)
- `json` → `to_json` · `txt` → `to_txt` (from port-output-json)

Carry the format on the job (`JobOpts`) so the queued (non-`--sync`) path also
emits the right format, and map it to the file extension when writing.

## where
- `rust/crates/submate-cli/src/main.rs` — `OutputFormat` clap enum + `-F/--format`
  on `TranscribeArgs`; extension selection in the `--sync` write path
  (replace the hardcoded `.srt`); thread the format into `JobOpts`.
- `rust/crates/submate-node/src/lib.rs` — the `whisper_processor` assembly step:
  emit the requested format instead of the hardcoded `to_srt_vtt(false)`.
- `rust/crates/submate-proto` / `JobOpts` — add the `output_format` field if the
  job struct needs it to reach the node.

## why
"ASS and JSON output instead of only SRT" is the whole point — the serializers
(port-output-ass/json) are useless until the CLI can select them and write the
correct file. This is the user-facing wiring.

## falsifies
`cargo test -p submate-cli` green, including:
- `output_format_extension_mapping`: each `OutputFormat` variant maps to the
  correct extension (`srt→.srt`, `vtt→.vtt`, `ass→.ass`, `json→.json`, `txt→.txt`)
  and to the correct serializer selector.
- `output_format_default_is_srt`: omitting `--format` yields `srt`/`.srt`
  (no behavior change for existing usage).
- a parse test that `-F ass`/`--format json` parse to the right variant and an
  invalid value is rejected.
