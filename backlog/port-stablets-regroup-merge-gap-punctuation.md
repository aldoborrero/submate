# Port stable-ts regroup B2: merge_by_gap (mg) + merge_by_punctuation (mp)

**blocked-by:** port-stablets-regroup-split-gap-punctuation

## what
Implement the `merge_by_gap` and `merge_by_punctuation` apply methods in
`apply_regroup_op` (`regroup.rs`). These are *merge* ops — they fuse adjacent
segments rather than split a segment's words — so unlike sg/sp/sl they do not
reuse `split_segments`; they need new segment-merging machinery (concatenate two
neighbouring segments' word lists, recompute the merged segment's start/end/text,
drop the absorbed segment, re-id).

Port upstream `WhisperResult.merge_by_gap(min_gap=0.3, max_words, max_chars,
is_sum_max=False, lock, newline)` — merge a segment into the next when the gap
between them is below `min_gap`, subject to the `max_words`/`max_chars` cap
(interpreted as a per-segment ceiling, or as the *sum* of both when
`is_sum_max=True`) — and `merge_by_punctuation(punctuation, max_words, max_chars,
is_sum_max, lock, newline)` — merge across a boundary when the earlier segment
ends with (or the later begins with) one of the `punctuation` tokens (same
string-or-`[prefix,suffix]` token shape as sp). Respect `lock` (don't merge
locked boundaries) and emit the upstream history string (`mg=...` / `mp=...`).

`mg=.3+3` is the fourth op of the upstream `da` default
(`DEFAULT_ALGO = "cm_sp=,* /，_sg=.5_mg=.3+3_sp=.* /。/?/？"`), so a `da` (or any
merge-containing) CUSTOM_REGROUP currently errors with `UnsupportedMethod`.
This is gated behind the split ops only for sequencing (`da` runs sp→sg→mg→sp,
so the split goldens should land first); the merge code itself is independent.

## where
`rust/crates/stable-ts/src/regroup.rs` — add `merge_by_gap` /
`merge_by_punctuation` fns + the segment-merge helper, and two arms in
`apply_regroup_op`.

## why
The upstream `da` default and any merge-based CUSTOM_REGROUP must regroup
identically to Python. Merge ops are the last gap in B2 once split ops land;
together they make every method in the `da` expansion executable.

## falsifies
`cargo test -p stable-ts parity::regroup_apply_merge` — applying `mg=.3+3` (and
`mp=.* /。/?/？`) in isolation to a fresh `WhisperResult` rebuilt from
`fixtures/stablets/clipA/00_raw.json` reproduces a new golden byte-for-byte via
`assert_json_eq`.

requires fixture: `rust/fixtures/stablets/clipA/01c_regroup_mg.json` (and
optionally `01c_regroup_mp.json`) (capture first — denylisted). Extend
`capture_stablets.py` to apply `mg=.3+3` in isolation from `00_raw` and dump
`to_dict()`. Until then this item is fixture-blocked.
