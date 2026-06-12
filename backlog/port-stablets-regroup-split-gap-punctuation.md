# Port stable-ts regroup B2: split_by_gap (sg) + split_by_punctuation (sp)

## what
Implement the `split_by_gap` and `split_by_punctuation` apply methods in
`apply_regroup_op`. Today that dispatcher (`regroup.rs`) only handles `clamp_max`
(`cm`) and `split_by_length` (`sl`); every other parsed method falls through to
`Err(UnsupportedMethod(...))`. The parser (B1) already knows all method codes
and binds their kwargs — `sg` → `split_by_gap(max_gap, lock, newline)` and
`sp` → `split_by_punctuation(punctuation, lock, newline, min_words, min_chars,
min_dur)` (see the `METHODS` table) — so this is purely the apply side.

Both are split ops and reuse the existing split machinery: `split_segments`
(the generic `get_indices`→split driver), `apply_newline`, `get_locked_indices`,
and `push_history`. Port upstream `Segment.get_gap_indices` (split where the gap
between word i.end and word i+1.start exceeds `max_gap`, default `0.1`) and
`Segment.get_punctuation_indices` (split after a word whose text ends with one of
the `punctuation` tokens — note the tokens may be `["x", "y"]` strings or
`[".", "* "]` two-element `[prefix, suffix]` lists coming out of
`str_to_valid_type`'s `/`/`*` parsing — gated by `min_words`/`min_chars`/
`min_dur` on the resulting pieces). Emit the same dotted history string
(`sg=...` / `sp=...`) upstream appends.

These are the first two ops of the upstream **`da`** default expansion
(`DEFAULT_ALGO = "cm_sp=,* /，_sg=.5_mg=.3+3_sp=.* /。/?/？"`), so any user who
sets `SUBMATE__STABLE_TS__CUSTOM_REGROUP` to a punctuation/gap algo (or `da`)
currently panics-by-error instead of regrouping.

## where
`rust/crates/stable-ts/src/regroup.rs` — add `split_by_gap` / `split_by_punctuation`
fns and wire two new arms into `apply_regroup_op`.

## why
Without sg/sp the only configurable regroup strings that work are length-based.
The upstream default algorithm and the documented `da` shorthand both depend on
these ops; a CUSTOM_REGROUP using them must regroup identically to Python, not
error out.

## falsifies
`cargo test -p stable-ts parity::regroup_apply_split` — applying `sg=.5` and
`sp=,* /，` each in isolation to a fresh `WhisperResult` rebuilt from
`fixtures/stablets/clipA/00_raw.json` reproduces a new golden byte-for-byte via
`assert_json_eq`.

requires fixture: `rust/fixtures/stablets/clipA/01b_regroup_sg.json` and
`rust/fixtures/stablets/clipA/01b_regroup_sp.json` (capture first — denylisted).
The existing `capture_stablets.py` only stages the submate default
`cm_sl=84_sl=42++++++1` ops; extend it to also apply `sg=.5` and the `da`-style
`sp=,* /，` op in isolation from `00_raw` and dump each `to_dict()`. Until then
this item is fixture-blocked.
