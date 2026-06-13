# perf: drop O(n²) cue rebuild and per-piece segment clones in stable-ts

## what
Two allocation/complexity hot spots in the (pure, non-`model`) stable-ts layer;
both must leave output **byte-identical**.

1. `stable-ts/src/output.rs` (`words2segments`, ~line 161-197): for word-level
   SRT/VTT it rebuilds the *entire* cue string once per word — O(n²) string
   concatenation per segment (`filled.iter().enumerate().map(..).collect()` inside
   a per-word loop, tagging only index `i`). Build the joined base once, then for
   each highlighted word splice only that word's tag into a clone — O(n) total.
2. `stable-ts/src/regroup.rs` (`split_segment`, ~line 646-678): each produced
   sub-segment does `piece = seg.clone()` — cloning the parent's **whole** word
   vector + metadata — then overwrites `piece.words` with a slice. For a segment
   split into k pieces that clones the full word list k times before discarding
   most of it. Clone only the per-segment metadata and move/clone the relevant
   word slice once per piece (e.g. `std::mem::take` the words up front and
   distribute, or build pieces from word-slice ranges).

No behavioral change — the emitted segments/cues are the same; only the work to
produce them shrinks.

## where
- `rust/crates/stable-ts/src/output.rs` — `words2segments` (word-level path).
- `rust/crates/stable-ts/src/regroup.rs` — `split_segment`.

## why
`words2segments` O(n²) hits every word-level SRT/VTT export; `split_segment`'s
full-word-vector clone-per-piece is paid by every split op (`sl`/`sg`/`sd`),
which the default regroup runs repeatedly.

## falsifies
`cargo test -p stable-ts` green with **unchanged** goldens — `parity::output`
(segment- and word-level SRT/VTT against `03.srt`/`03.vtt`) and `parity::regroup`
(all `01_regroup_*` goldens) must still pass byte-for-byte, proving identical
output. Add a word-level `to_srt_vtt(word_level=true)` assertion if one is not
already covered. `cargo clippy -p stable-ts --all-targets -D warnings` clean.
