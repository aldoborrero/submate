# tried: port-subtitle-srt-vtt-parse

## outcome
Abandoned — scope violation (denylist hit).

## what happened
The grind branch `grind/port-subtitle-srt-vtt-parse` modified a file outside the
item's allowed scope:

- `rust/fixtures/README.md`

Everything under `rust/fixtures/` is golden parity data and is on the denylist.
A porter editing its own fixtures (or the docs that describe them) defeats the
parity guarantee, so the branch could not be auto-applied and was rejected.

## actions taken
- Removed worktree `port-subtitle-srt-vtt-parse` and deleted branch
  `grind/port-subtitle-srt-vtt-parse`.
- Rerouted the item from `backlog/port-subtitle-srt-vtt-parse.md` to
  `backlog/needs-human/port-subtitle-srt-vtt-parse.md`. Triage skips `backlog/`
  subdirectories, so the item will not be auto-picked again until a human acts
  on it.

## next steps (human)
A human reviews `backlog/needs-human/port-subtitle-srt-vtt-parse.md` and either:

1. applies the denylisted change directly — authors the `rust/fixtures/README.md`
   change (and any other `rust/fixtures/` goldens) themselves, then re-runs the
   item, or
2. re-scopes the item so it avoids `rust/fixtures/` (e.g. land any fixture/doc
   changes in a separate human-owned step first) and moves it back to `backlog/`
   for automated pickup, or
3. deletes the item if it is no longer wanted.
