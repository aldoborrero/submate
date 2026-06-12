# bug parity (stable-ts): `clamp_max` median index and word-count gate diverge from Python; clipA golden masks both

**related:** parity-stablets-regroup-stage-false-coverage (the `regroup_apply`
falsifier it asked for now exists and passes — but passes *despite* this bug,
because the only clamp_max golden is a no-op)

## what
`rust/crates/stable-ts/src/regroup.rs::clamp_max` is not a faithful port of
`WhisperResult.clamp_max` (stable-ts 2.19.1 `result.py:2016`). Two divergences:

1. **Median index off by one.** Python:
   ```python
   durations.sort()
   curr_max_dur = medium_factor * durations[len(durations)//2]   # index = n//2
   ```
   Rust:
   ```rust
   durations.sort_by(...);
   curr_max_dur = Some(factor * durations[durations.len() / 2 + 1]);  // index = n/2 + 1
   ```
   The Rust comment above it even claims `Python durations[len//2 + 1]` — that is
   factually wrong; the Python source has **no `+ 1`**. For an even-length array
   numpy/Python still index a single element here (this is a raw index, not a
   true median average), so `n//2` is the contract. Rust picks the element one
   slot higher, giving a larger cap.

2. **Word-count gate.** Python clamps when `len(seg.words) > 1` (i.e. >= 2
   words); Rust gates on `words.len() > 2` (i.e. >= 3 words). A 2-word segment
   is clamped by Python but skipped by Rust.

## where
- bug: `rust/crates/stable-ts/src/regroup.rs`, fn `clamp_max`
  - line ~381: `if words.len() > 2` should be `if words.len() > 1`
  - line ~384-385: comment + `durations[durations.len() / 2 + 1]` should be
    `durations[durations.len() / 2]`
- spec: stable-ts 2.19.1 `result.py:2053-2058`
  (`.cache/uv/.../stable_whisper/result.py`)
- masking golden: `rust/fixtures/stablets/clipA/01_regroup_0_clamp_max.json`
- driving test: `rust/crates/stable-ts/tests/parity.rs::regroup_apply` (op i=0)

## golden truth — exact diff (Python vs Rust cap, per clipA segment)
Computed from `00_raw.json` word durations (`medium_factor=2.5`, default
`max_dur=None`, `clip_start=None`):

| segment | n words | Python idx (`n//2`) → val | **Python cap** | Rust idx (`n//2+1`) → val | **Rust cap** |
|---------|---------|---------------------------|----------------|---------------------------|--------------|
| seg0    | 17      | 8 → 0.20                  | **0.500**      | 9 → 0.24                  | **0.600**    |
| seg1    | 12      | 6 → 0.24                  | **0.600**      | 7 → 0.26                  | **0.650**    |

The caps differ in both segments. The defect does **not** surface in the
`01_regroup_0_clamp_max` golden only because, with `clip_start=None`, clamp_max
touches solely the first word's start and the last word's end, and on clipA
neither edge word exceeds *either* cap:

| segment | first-word dur | last-word dur | clamped under Python cap? | under Rust cap? |
|---------|----------------|---------------|---------------------------|-----------------|
| seg0    | 0.14           | 0.32          | no (both < 0.500)         | no              |
| seg1    | 0.60           | 0.52          | no (0.60 == cap, < )      | no              |

So `01_regroup_0_clamp_max.json` is **byte-identical** to `00_raw.json`
(segments + text deep-equal), and `regroup_apply` op i=0 passes even though the
ClampMax transform — cap derivation included — is wrong. The `clamp_word`
arithmetic (`set_start(end - cap)` / `set_end(start + cap)`) is likewise never
exercised: no edge word in any clamp golden exceeds its cap.

## falsifies
A clamp_max parity test must exercise a segment where the **first or last** word
duration falls between the Python cap and the Rust cap (or where the word-count
gate flips a 2-word segment), so the wrong index / wrong gate produces a visibly
different word timing. Two options:

1. Capture a new golden `01_regroup_0b_clamp_max.json` from a fresh
   `WhisperResult` whose first/last word duration exceeds the Python `n//2` cap
   but not the Rust `n//2+1` cap (e.g. a hand-built 3-word and a 2-word segment),
   dump `to_dict()` from real Python `WhisperResult.clamp_max(2.5)`, and add
   `parity::clamp_max_edge` asserting the Rust `apply_regroup_op` / `clamp_max`
   matches it exactly. With the current Rust code that test FAILS; after fixing
   index → `n//2` and gate → `> 1` it PASSES.

2. Cheaper sibling unit test in `regroup.rs` (no golden): build a `WhisperResult`
   with one 2-word segment (durations `[0.1, 0.9]`) and one 3-word segment
   (durations `[0.2, 0.2, 0.9]`, sorted `[0.2,0.2,0.9]`, Python cap `2.5*0.2=0.5`),
   call `clamp_max(Some(2.5), None, None)`, and assert the last word's end is
   clamped to `start + 0.5` and the 2-word segment is clamped (not skipped).
   Current Rust skips the 2-word seg (gate) and uses cap `2.5*0.9=2.25` for the
   3-word seg (index), so nothing clamps → test FAILS until both lines are fixed.

Prefer option 1 (golden-pinned, matches the parity contract) but option 2 is
enough to falsify the two arithmetic defects in isolation.
