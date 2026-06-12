# parity (stable-ts): `regroup_stage_roundtrip` gives false regroup coverage

**related:** port-stablets-regroup-splits-B2 (the missing transform this should guard)

## what
`rust/crates/stable-ts/tests/parity.rs::regroup_stage_roundtrip` is named like a
regroup parity test but does NOT exercise the regroup transform. It loads each
**output** golden (`01_regroup_0_clamp_max.json`,
`01_regroup_1_split_by_length.json`, `01_regroup_2_split_by_length.json`) and
asserts `WhisperResult::from_value(golden).to_dict() == golden` — a pure
serialization roundtrip. It never reads `00_raw.json` and never applies
`clamp_max` / `split_by_length`, so it stays green even though the regroup
transform is **unimplemented** (`rg 'clamp_max|split_by_length' on
`stable-ts/src/` finds only the parser table in `regroup.rs`, no transform on
`WhisperResult`).

The risk: a scan of "is stable-ts regroup parity covered?" sees a green
`regroup_stage_roundtrip` next to `regroup_parse` and concludes the staged
goldens are checked. They are not. B2 can land wrong (or never land) and this
test will not catch it.

## where
- `rust/crates/stable-ts/tests/parity.rs` (the misleading test)
- goldens: `rust/fixtures/stablets/clipA/00_raw.json` (input),
  `rust/fixtures/stablets/clipA/01_regroup_{0_clamp_max,1_split_by_length,2_split_by_length}.json`
  (per-op outputs)
- capture intent: `rust/fixtures/capture/capture_stablets.py` lines 70-77 —
  each `01_regroup_<i>_<method>.json` is `WhisperResult(raw_dict)` with op `i`
  applied **in isolation from a fresh raw** (stages do not compound). The
  capture header (line 9) explicitly names the intended test:
  `stable-ts parity::regroup_apply  <- 01_regroup_<op>.json`.

## golden truth (what the real test must produce)
Per-op segment shape after applying each op in isolation to `00_raw` (2 segs,
29 words):
- `00_raw`                     → 2 segments
- `01_regroup_0_clamp_max`     → 2 segments (word timings clamped; same seg count)
- `01_regroup_1_split_by_length` → 3 segments (split by max_chars=84)
- `01_regroup_2_split_by_length` → 2 segments (split by max_chars=42, newline=1)

`clamp_max` mutates word start/end without changing segment count;
`split_by_length` changes segment count. `regroup_stage_roundtrip` is blind to
all of this because it round-trips the output, not the transform.

## falsifies
`cargo test -p stable-ts parity::regroup_apply` exists and, for each `i` in the
parsed `parse_regroup_algo("cm_sl=84_sl=42++++++1")` op list, applies op `i`
in isolation to a fresh `WhisperResult::from_value(00_raw.json)` and asserts the
result `to_dict()` equals `rust/fixtures/stablets/clipA/01_regroup_<i>_<method>.json`
exactly (`parity::assert_json_eq`).

This is the same falsifier B2 carries — filing it here because the **existing**
`regroup_stage_roundtrip` test actively masks the gap and should be deleted or
renamed (it only proves model serialization, which `model_roundtrip` /
`suppress_roundtrip` already cover) once `regroup_apply` lands. Until then, do
not trust `regroup_stage_roundtrip` as regroup-parity evidence.
