# spike: can `subparse` round-trip a tagged .ass losslessly?

> **Investigation, not production code.** This is a time-boxed evaluation whose
> deliverable is a *recommendation* (adopt `subparse` for the ASS translation
> path, or hand-roll per the port plan). It is intentionally OUTSIDE the
> `port-output-` focus — it concerns ASS *parsing* on the translation path, not
> ASS *output*.

## what
The translation path must parse an existing fansub `.ass`, translate each
`Dialogue` line's *visible text* while preserving every `{\...}` override tag,
the `\N` hard breaks, the comment lines, styles, and overall structure, then
re-emit. The port plan assumed we'd hand-roll this because general subtitle
libraries normalize/reformat on re-emit and can drop or mangle inline tags.

Before committing to a hand-rolled ASS parser, evaluate `subparse` (v0.7,
"load, change and write srt/ass/idx/sub") against that exact bar.

Method:
1. Add `subparse` as a **dev-dependency** of `submate-subtitle` (spike only —
   do not wire it into the library API yet).
2. In a `#[test]` (or `examples/`) load `rust/fixtures/subtitle/tagged_sample.ass`,
   edit each dialogue line's visible text (e.g. uppercase it, simulating a
   translation), write back to a string, and compare to the input.
3. Record, in the spike's resolution, whether these survive an edit+re-emit:
   - inline override tags: `{\i1}...{\i0}`, `{\pos(960,200)\c&H00FFFF&}`, `{\k50}`
   - `\N` hard line breaks inside text
   - the `Comment:` line
   - the two `Style:` lines and `[Script Info]` resolution
   - whether *only* the visible text changed and tags/structure are byte-stable

## where
- `rust/crates/submate-subtitle/` — dev-dependency + a spike test/example.
- Fixture (already committed): `rust/fixtures/subtitle/tagged_sample.ass`.

## why
If `subparse` preserves tags on edit, adopting it saves writing and maintaining
a tag-aware ASS parser for `port-subtitle-ass-tags` /
`port-translate-ass-tag-validate`. If it mangles tags, we confirm the hand-roll
decision with evidence instead of assumption.

## falsifies
Resolve to ONE, recorded in this item (or a short `rust/docs/` note) before the
file is removed:

1. **Adopt.** A spike test demonstrates an edit-and-re-emit of
   `tagged_sample.ass` that changes only the visible dialogue text and leaves
   all override tags, `\N`, the `Comment:` line, and styles intact. Then
   `port-subtitle-ass-tags` is rewritten to build on `subparse`.
2. **Hand-roll.** The spike shows `subparse` drops/reformats tags or otherwise
   fails the round-trip; record the specific failure (which tag, what it became)
   so `port-subtitle-ass-tags` proceeds hand-rolled with that evidence.
