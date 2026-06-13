# perf: quickselect instead of full sort for k-th-largest / median

## what
Two hot spots fully sort a buffer only to read one element; replace with
`select_nth_unstable` (quickselect, O(n) avg) — output is **identical**, just
cheaper.

1. `stable-ts/src/suppress_silence.rs` (`audio2loudness`, ~line 51/57): clones the
   entire abs-sample buffer and `sort_by`s it (O(n log n) over the whole clip's
   PCM — millions of samples) just to read the k-th-largest. Use
   `select_nth_unstable_by` to partition at `k` in O(n) and drop the extra clone.
2. `stable-ts/src/regroup.rs` (`clamp_max`, ~line 382): allocates + sorts a fresh
   per-segment duration vector each pass only to take the median
   (`durations[len/2]`). Use `select_nth_unstable_by(len/2, ..)`.

Pure refactor: the selected value at the k-th position is the same one the sort
would place there, so every downstream timing is unchanged. Neither file is
behind the `model` feature, so the gate fully exercises this.

## where
- `rust/crates/stable-ts/src/suppress_silence.rs` — `audio2loudness`.
- `rust/crates/stable-ts/src/regroup.rs` — `clamp_max`.

## why
`audio2loudness` runs once over the entire decoded clip; the O(n log n) sort + a
full clone is the single largest avoidable allocation/CPU in the suppress path.
`clamp_max` sorts per segment per pass. Both are O(n)-replaceable with no output
change.

## falsifies
`cargo test -p stable-ts` green with the existing goldens **unchanged** —
`parity::suppress` (against `02_suppress.json`) and `parity::regroup` (against
`01_regroup_0_clamp_max.json`) must still pass byte-for-byte after the swap,
proving output is identical. `cargo clippy -p stable-ts --all-targets -D
warnings` clean (no `select_nth` misuse / unused sort imports).
