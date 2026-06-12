# Port stable-ts regroup B2: split_by_duration (sd) + merge_all_segments (ms)

blocked-by: none (B1 parse + the shared split machinery are already in `regroup.rs`)

## what
Implement the `split_by_duration` and `merge_all_segments` apply methods in
`apply_regroup_op` (`rust/crates/stable-ts/src/regroup.rs`). Today that
dispatcher handles only `clamp_max` (`cm`) and `split_by_length` (`sl`); every
other parsed method — including `sd` and `ms` — falls through to
`UnsupportedMethod`. B1 (`parse_regroup_algo`) already recognises both codes and
binds their kwargs:

- `sd` → `split_by_duration(max_dur, even_split, force_len, lock, include_lock, newline)`
- `ms` → `merge_all_segments()` (no kwargs)

`split_by_duration` is a split op in the same family as `sl`/`sg`: it reuses the
existing split machinery (`split_segments`, the generic `get_indices`→split
driver, `apply_newline`, `get_locked_indices`, `push_history`). Port upstream
`Segment.get_duration_indices` (split where cumulative word duration from the
last split point would exceed `max_dur`; with `even_split=True` it distributes
splits evenly like `split_by_length` does for chars). Defaults from
`WhisperResult.split_by_duration`: `max_dur=None`, `even_split=True`,
`force_len=False`, `lock=False`, `include_lock=False`, `newline=False`. When
`max_dur` is `None` upstream raises — match whatever the existing `sl` arm does
for its required arg (treat absent `max_dur` as a no-op or mirror the upstream
error path; verify against the captured fixture).

`merge_all_segments` collapses every segment into a single segment (concatenate
all words in order into one `Segment`, recompute its `start`/`end`/`text` the
same way the existing merge helpers do, and `push_history`). No kwargs to bind.

Wire two new arms into `apply_regroup_op` and keep the per-op history-append
side effect (`sd=...` / `ms`) that upstream `regroup` appends to `regroup_history`
identically to the `cm`/`sl` arms.

## where
`rust/crates/stable-ts/src/regroup.rs` — add `split_by_duration` /
`merge_all_segments` fns plus the two `apply_regroup_op` match arms. Reuse the
existing `split_segments` / `apply_newline` / merge helpers; do not duplicate
them.

## why
Independent of the open `sg`/`sp` and `mg`/`mp` items (different method codes,
separate arms, no shared new code). `sd` and `ms` are the last two members of
the split/merge apply family that parse (B1) but don't run (B2). Neither appears
in `DEFAULT_ALGO` (`cm_sp_sg_mg_sp`) nor in submate's config default
(`cm_sl=84_sl=42++++++1`), so they're reachable only via an explicit user
`custom_regroup` string — but the moment a user sets e.g. `cm_sd=4` or
`ms` the pipeline must run it rather than error, and the result must match the
Python golden byte-for-byte (regroup is an **exact**-parity layer per
`rust/fixtures/README.md`).

## falsifies
`cargo test -p stable-ts parity::regroup_apply_duration_merge` — rebuild a fresh
`WhisperResult` from `fixtures/stablets/clipA/00_raw.json`, apply `sd=4` in
isolation, and assert the `to_dict()` JSON matches the captured golden
byte-for-byte via `parity::assert_json_eq`; repeat for `ms` applied to a fresh
`00_raw`. Mirror the existing `01_regroup_<i>_<fn>.json` shape the
`regroup_apply` test already consumes.

requires fixture: `rust/fixtures/stablets/clipA/01c_regroup_sd.json` and
`rust/fixtures/stablets/clipA/01c_regroup_ms.json` (capture first — fixtures dir
is denylisted to the scout). Capture by extending
`rust/fixtures/capture/capture_stablets.py`'s "apply each op in isolation" loop
to also run `fresh.split_by_duration(max_dur=4)` and a separate
`fresh.merge_all_segments()` against a fresh `00_raw` `to_dict()`, dumping each
to the filenames above. Use the same `tiny` model / clipA wav the existing
stablets fixtures were captured from so `00_raw` is identical.
