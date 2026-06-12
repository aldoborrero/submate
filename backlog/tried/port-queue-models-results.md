# tried: port-queue-models-results

## outcome
Abandoned (again) — scope violation (denylist hit). Item stays in `backlog/`.

## what happened
The grind branch `grind/port-queue-models-results` modified a file outside the
item's allowed scope:

- `rust/fixtures/capture/capture_queue.py`

This path is on the denylist. Capture inputs are golden parity data: the porter
is not allowed to author/drive the capture script or write the goldens under
`rust/fixtures/`. The item itself calls for capturing the five canonical
task-envelope JSON objects into `rust/fixtures/queue/task_envelopes.json` (no
`rust/fixtures/queue/` dir exists yet) by adding/driving a capture script
`capture_queue.py`. Because the porter touched the denylisted capture file, the
branch could not be auto-applied and was rejected.

The branch also touched the legitimate target files
(`rust/crates/submate-queue/src/models.rs`, `lib.rs`, `Cargo.toml`,
`tests/parity.rs`) plus `rust/fixtures/queue/task_envelopes.json` and
`rust/fixtures/capture/run_deterministic.sh`, all dependent on the denylisted
capture step. The whole branch was discarded.

## actions taken
- Removed worktree `port-queue-models-results` and deleted branch
  `grind/port-queue-models-results` (was `f97be4a`).
- **Did NOT re-park the item to `backlog/needs-human/`.** This is a *pure-data*
  capture (`submate.queue.models` imports cleanly in the nix devshell — no
  Whisper model, audio, live server, credential, or network needed). Per the
  documented triage rule in `backlog/meta-contention.md` and
  `backlog/meta-capture-prepass.md`, pure-data capture-blocked items belong in
  `backlog/`, to be authored by the **capture pre-pass** before dispatch — they
  do NOT go to `needs-human/`. This exact item is called out as a chronic
  re-park anti-pattern; commit `1eb1ca8` deliberately unparked it back to
  `backlog/` with the note "do NOT re-park to needs-human/". So
  `backlog/port-queue-models-results.md` is left in `backlog/` untouched.

## why the porter keeps tripping
The item's falsifier is blocked on a golden that does not exist yet, and the
item's own text points the porter at `capture_queue.py`. The porter follows
that pointer and authors the denylisted capture script, which trips the
denylist every time. The structural fix is the **enforced capture pre-pass**
(see `backlog/meta-capture-prepass.md`): author the capture script, run it, and
land `rust/fixtures/queue/task_envelopes.json` in a deliberate capture commit
*before* this item is dispatched. Once the golden exists, the porter only edits
`rust/crates/submate-queue/` and stays in scope.

## next steps
- Capture pre-pass (META, before next dispatch): author
  `rust/fixtures/capture/capture_queue.py`, drive the real `registered_tasks`
  paths where feasible (else construct from `TaskResult` /
  `TranscriptionSkippedError`), emit the five canonical envelope JSON objects
  into `rust/fixtures/queue/task_envelopes.json`, commit as a capture commit.
- Then re-dispatch `port-queue-models-results` from `backlog/`; the porter
  implements only `rust/crates/submate-queue/src/models.rs` + parity test.
