# cli-audio: TTY-gated interactive track picker

**blocked-by:** cli-audio-probe-command, cli-audio-selector-grammar

## what
When transcribing a **single** file whose track selection is ambiguous, and a
human is present, prompt them to pick a track instead of silently guessing.
"Ambiguous" = a `Lang` selector matched more than one track, OR no selector was
given and the file has several tracks with no default-disposition track.

The decision must be a **pure function**, with the stdin read/render as thin I/O
around it:

```
enum TrackDecision { Resolved(usize), Prompt(Vec<usize>), Error(String) }
fn decide_track(tracks, sel: Option<&AudioSelector>, is_tty: bool, non_interactive: bool) -> TrackDecision
```

Rules:
- Selector resolves unambiguously (incl. `Auto` with a clear default, single
  track) → `Resolved(index)`.
- Ambiguous AND `is_tty` AND `!non_interactive` → `Prompt(candidate indices)`
  → the I/O layer renders the candidates (reuse `render_track_table` from
  cli-audio-probe-command) and reads a choice.
- Ambiguous AND (not a TTY OR `--non-interactive`) → `Resolved(rule pick)`
  (first match / track 0) and the caller logs a one-line note naming the pick
  and how to override. **Never block on a pipe, batch, or server.**

Add a `--non-interactive` flag (alias `--yes`) to force the rule path. Only the
single-file path prompts; multi-file/recursive runs always take the rule.

## where
- `rust/crates/submate-cli/src/main.rs` — `decide_track` (pure) + `--non-interactive`
  on `TranscribeArgs`; gate the prompt on `std::io::stderr().is_terminal()`
  (same pattern as `ProgressRenderer::for_stderr`); render via `render_track_table`.

## why
This is the "I'm not sure what's in this file" moment — a numbered prompt is the
right affordance interactively, but it must degrade to a deterministic rule the
instant no human can answer (pipe, batch, `--non-interactive`, headless). One
mental model, three contexts.

## falsifies
`cargo test -p submate-cli` green, including `decide_track_*`:
- unambiguous selector → `Resolved(expected_index)` regardless of `is_tty`.
- ambiguous + `is_tty=true` + `non_interactive=false` → `Prompt([..candidates])`.
- ambiguous + `is_tty=false` → `Resolved(rule_index)` (no prompt off a TTY).
- ambiguous + `is_tty=true` + `non_interactive=true` → `Resolved(rule_index)`.
