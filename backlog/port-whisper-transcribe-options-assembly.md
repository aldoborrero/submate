# Port whisper.py transcribe option-assembly precedence + task validation

**blocked-by:** port-whisper-pipeline

## what
`WhisperModelWrapper.transcribe` (submate/whisper.py) builds the kwargs map
handed to `transcribe_stable` with a precise override precedence, plus a task
validation guard. None of this is ported: the Rust `TranscribeOptions`
(rust/crates/submate-whisper/src/lib.rs) carries only `language` + `task` and
never plumbs `config.whisper.transcribe_kwargs` through, so any operator
`beam_size`/`best_of`/etc. is silently dropped.

Port the assembly as a pure, deterministic function that mirrors lines 226-247
of whisper.py exactly:

- `pub fn build_transcribe_options(config_kwargs, task, language, extra) -> Map`
  - Start from `config.whisper.transcribe_kwargs` (a JSON object; already
    modeled in submate-config as `Map<String, Value>`).
  - Insert `"task"` = the task `.value` string (`"transcribe"` | `"translate"`)
    — this OVERWRITES any `task` already present in config_kwargs (Python:
    `options["task"] = task`).
  - If `language` is `Some` and non-empty, insert `"language"`; if `None`, do
    NOT insert it (Python: `if language:` — empty string is falsy and must be
    omitted, leaving auto-detect).
  - Merge `extra` last with `options.update(extra)` semantics: extra kwargs
    override EVERYTHING, including the just-set `task`/`language`.
  - Result key order is irrelevant (compare as a map, not byte string), but
    the resolved value for every key must match Python.
- Task validation: before assembly, reject any task not in `VALID_TASKS`
  (`{"transcribe", "translate"}`) with the byte-exact message
  `Invalid task: {task}. Valid options: {joined}`. In Rust the `Task` enum
  already constrains the typed path, but the parity surface is the *string*
  entry point (the CLI/Bazarr/queue layers pass a task string), so expose a
  `Task::from_validated_str(&str) -> Result<Task, String>` whose `Err` is that
  exact sentence. NOTE: Python joins `VALID_TASKS` from a **set**, so the order
  of the two options in the message is nondeterministic across runs — the
  falsifier must assert the prefix `Invalid task: invalid. Valid options: `
  and that both `transcribe` and `translate` appear, NOT a fixed full string.

This is the only place config-level transcription tuning reaches the model;
it is pure-data so the resolved kwargs must match the Python golden exactly.

## where
`rust/crates/submate-whisper/src/lib.rs` (alongside `TranscribeOptions`), plus
its parity test `rust/crates/submate-whisper/tests/parity.rs`.

## why
Operators set `SUBMATE__WHISPER__TRANSCRIBE_KWARGS='{"beam_size":5}'` to tune
decoding. Python merges these into every transcribe call with extra-kwargs >
task/language > config precedence. The current Rust pipeline ignores them
entirely, so transcription quality diverges from the Python spec for any
non-default config. The precedence (extra overrides task, language omitted when
empty) is subtle and a wrong order silently changes inference behavior.

## falsifies
`cargo test -p submate-whisper parity::transcribe_options` asserts the resolved
option map for a matrix of golden cases under
`rust/fixtures/transcribe/options/` (one JSON file per case, the dict Python
would pass to `transcribe_stable` minus the `regroup=` arg which is positional):

- `base.json`: config_kwargs `{"beam_size":5}`, task=transcribe, language=None,
  extra empty → `{"beam_size":5,"task":"transcribe"}` (no `language` key).
- `lang.json`: same + language="en" → adds `"language":"en"`.
- `lang_empty.json`: language="" → NO `language` key (falsy-omit).
- `extra_override.json`: config_kwargs `{"task":"translate","beam_size":5}`,
  task=transcribe, extra `{"beam_size":1,"task":"transcribe"}` →
  `{"task":"transcribe","beam_size":1}` (extra wins over config's `task`).

Plus a non-golden unit assertion: `Task::from_validated_str("invalid")` is
`Err` whose string starts `Invalid task: invalid. Valid options: ` and contains
both `transcribe` and `translate`; `from_validated_str("transcribe")` and
`("translate")` are `Ok`.

requires fixture: rust/fixtures/transcribe/options/{base,lang,lang_empty,
extra_override}.json — capture by instrumenting `WhisperModelWrapper.transcribe`
to dump the assembled `options` dict (the local built at whisper.py:242-247,
before `transcribe_stable`) as JSON for each input case. I cannot touch
rust/fixtures/ (denylisted); flag for human capture before the implementer
starts.
