# tried: port-subtitle-srt-vtt-parse

## outcome
Abandoned — scope violation (denylist hit).

## what happened
The grind branch `grind/port-subtitle-srt-vtt-parse` modified a file outside the
item's allowed scope:

- `rust/fixtures/README.md`

`rust/fixtures/` is `mergeDenylist`-protected: the subtitle goldens and the
README that lists them are golden parity data that must be authored by a human or
a dedicated capture commit, not by an automated port item. Letting the porter
write the fixtures (or the README that indexes them) makes the parity check
self-referential. The item itself flags this in its "capture precondition"
section, and a prior attempt was already rerouted for the same reason. Because
the branch touched the denylisted `rust/fixtures/README.md`, it could not be
auto-applied and was rejected.

## actions taken
- Removed worktree `port-subtitle-srt-vtt-parse` and deleted branch
  `grind/port-subtitle-srt-vtt-parse`.
- Restored `backlog/port-subtitle-srt-vtt-parse.md` from `origin/main` and
  rerouted it to `backlog/needs-human/port-subtitle-srt-vtt-parse.md`. Triage
  skips `backlog/` subdirectories, so the item will not be auto-picked again.

## next steps (human)
A human reviews `backlog/needs-human/port-subtitle-srt-vtt-parse.md` and either:
1. applies the denylisted change directly — author `capture_subtitle.py`, capture
   the `rust/fixtures/subtitle/*.{srt,vtt}` round-trip goldens, and list them in
   `rust/fixtures/README.md` via a deliberate capture commit — then re-runs the
   item, or
2. re-scopes the item so it avoids the denylisted `rust/fixtures/` paths and moves
   it back to `backlog/` for automated pickup, or
3. deletes the item if it is no longer wanted.
