# tried: port-stablets-suppress-dsp-C1

## outcome
Abandoned — scope violation (denylist hit).

## what happened
The implementer branch `grind/port-stablets-suppress-dsp-C1` modified a file
outside the item's allowed scope:

- `rust/fixtures/stablets/clipA/audio.f32`

This path is on the denylist (capture/fixture binaries must not be authored or
altered by an automated port item — the item is expected to consume these
fixtures, not generate them), so the branch was rejected rather than merged.

## actions taken
- Removed worktree `port-stablets-suppress-dsp-C1` and deleted branch
  `grind/port-stablets-suppress-dsp-C1`.
- Rerouted the backlog item to
  `backlog/needs-human/port-stablets-suppress-dsp-C1.md`. Triage skips
  subdirectories, so the item will not be auto-picked again.

## next steps (human)
A human reviews `backlog/needs-human/port-stablets-suppress-dsp-C1.md` and either:
1. applies the denylisted change (the `audio.f32` fixture) directly, then
   re-runs the item, or
2. re-scopes the item to exclude the denylisted path and moves it back to
   `backlog/` for automated pickup, or
3. deletes the item if it is no longer wanted.
