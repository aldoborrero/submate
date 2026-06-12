# tried: port-subtitle-srt-vtt-parse

## outcome
Abandoned — scope violation (denylist hit).

## what happened
The grind branch `grind/port-subtitle-srt-vtt-parse` modified a file outside the
item's allowed scope:

- `rust/fixtures/subtitle/basic.srt`

This path is on the denylist. Subtitle fixtures are golden parity data and must
be authored by a human/capture run, not rewritten by an automated port item: the
item's falsifier re-emits each `rust/fixtures/subtitle/*.srt` byte-identically,
so the fixtures are the reference oracle, not editable output. Because the porter
touched the denylisted fixture, the branch could not be auto-applied and was
rejected.

## actions taken
- Removed worktree `port-subtitle-srt-vtt-parse` and deleted branch
  `grind/port-subtitle-srt-vtt-parse`.
- Restored `backlog/port-subtitle-srt-vtt-parse.md` from `origin/main` and
  rerouted it to `backlog/needs-human/port-subtitle-srt-vtt-parse.md`. Triage
  skips `backlog/` subdirectories, so the item will not be auto-picked again.

## next steps (human)
A human reviews `backlog/needs-human/port-subtitle-srt-vtt-parse.md` and either:
1. applies the denylisted change directly (author/capture the
   `rust/fixtures/subtitle/*.srt` fixtures by hand), then re-runs the item, or
2. re-scopes the item to exclude the denylisted fixture files and moves it back
   to `backlog/` for automated pickup, or
3. deletes the item if it is no longer wanted.
