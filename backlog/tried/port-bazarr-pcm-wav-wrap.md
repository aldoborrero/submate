# tried: port-bazarr-pcm-wav-wrap

## outcome
Abandoned — scope violation (denylist hit).

## what happened
The grind branch `grind/port-bazarr-pcm-wav-wrap` modified a file outside the
item's allowed scope:

- `rust/fixtures/bazarr/pcm/sine440.pcm`

Everything under `rust/fixtures/` is golden parity data and is on the denylist.
The item itself flagged this up front: the falsifier pins
`wrap_pcm_as_wav(...)` against `rust/fixtures/bazarr/pcm/sine440.wav` and the
backlog note explicitly said the `rust/fixtures/` goldens cannot be authored by
the porter (a porter writing its own oracle defeats the parity check) and must
be produced by a human or a deliberate capture pre-pass. The branch generated
and committed the `sine440.pcm` (and `sine440.wav`) fixtures itself, so it
could not be auto-applied and was rejected.

## actions taken
- Removed worktree `port-bazarr-pcm-wav-wrap` and deleted branch
  `grind/port-bazarr-pcm-wav-wrap`.
- Rerouted the item from `backlog/port-bazarr-pcm-wav-wrap.md` to
  `backlog/needs-human/port-bazarr-pcm-wav-wrap.md`. Triage skips `backlog/`
  subdirectories, so the item will not be auto-picked again until a human acts
  on it.

## next steps (human)
A human reviews `backlog/needs-human/port-bazarr-pcm-wav-wrap.md` and either:

1. applies the denylisted change directly — authors / captures the
   `rust/fixtures/bazarr/pcm/sine440.pcm` and `sine440.wav` goldens (e.g. via a
   deterministic `rust/fixtures/capture/capture_bazarr_audio.py` sine pre-pass),
   then re-runs the item, or
2. re-scopes the item so it avoids `rust/fixtures/` (e.g. land the goldens in a
   separate human-owned capture step first) and moves it back to `backlog/` for
   automated pickup, or
3. deletes the item if it is no longer wanted.
