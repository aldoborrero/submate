# Parity: transcribe success summary line diverges from Python

## what
The `transcribe --sync` per-file success line in the Rust CLI was redesigned and
no longer matches the Python golden behavior. There is **no golden fixture**
capturing this line, so the divergence is currently untested.

Python (`submate/cli/commands/transcribe.py`, `_process_files`, the success
branch) prints exactly, inside `[green]` Rich markup with two leading spaces:

```
  ✓ Processed: {file.name}
```

i.e. just a checkmark, the word `Processed:`, and the **input** file's basename.
(See lines around `console.print(f"  [green]✓[/green] Processed: {file.name}")`.)

Rust (`rust/crates/submate-cli/src/main.rs`, `result_summary`, introduced in
`ce54cd9 feat(cli): clean transcribe summary`) now prints:

```
✓ {input.basename} → {output.basename} ({n} cues)
```

e.g. `✓ movie.mkv → movie.srt (42 cues)` — no leading indent, an arrow to the
output filename, and a pluralized cue count that Python never emitted.

First differing tokens for input `movie.mkv` → `movie.srt`, 42 cues:

| | Python golden | Rust |
|---|---|---|
| leading indent | `  ` (2 spaces) | none |
| label | `✓ Processed: ` | `✓ ` |
| body | `movie.mkv` | `movie.mkv → movie.srt (42 cues)` |

## where
- `rust/crates/submate-cli/src/main.rs` — `result_summary` + its call site in
  `transcribe_files`.
- Python spec: `submate/cli/commands/transcribe.py` success branch of
  `_process_files`.
- New golden: `rust/fixtures/cli/transcribe_summary_cases.json` (capture from the
  Python success line for a small set of input basenames).

## why
CLI console "output formatting" is an exact-match parity layer per
`rust/fixtures/README.md`. The redesign may be an intentional UX improvement
(the cue count and output path are genuinely useful), but right now it is an
**undocumented, untested divergence** from the Python spec. The curator/aligner
must decide: either (a) Rust matches the Python `✓ Processed: {name}` line, or
(b) the enhanced summary is ratified as a deliberate divergence and recorded as
such so a future parity sweep does not flag it again.

## falsifies
Resolve to ONE of:

1. **Match Python.** Capture `rust/fixtures/cli/transcribe_summary_cases.json`
   from the Python success line, and add
   `cargo test -p submate-cli parity::transcribe_summary_cases` asserting
   `result_summary` (or its successor) reproduces `  ✓ Processed: {name}`
   byte-for-byte for each case. Green proves parity.

2. **Ratify divergence.** Add a documented exception (e.g. in
   `rust/fixtures/README.md` or a `rust/docs/` divergence note) stating the
   transcribe summary intentionally differs, with the Python-vs-Rust strings
   above, and keep the existing `result_summary_format` unit test as the spec.
   The falsifier is then: that divergence note exists and references this item.
