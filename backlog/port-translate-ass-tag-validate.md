# Port ASS tag-preservation validation + keep-original fallback

**blocked-by:** port-subtitle-ass-tags

## what
Port the ASS-tag-preservation guard from `submate/translation.py`:

- `validate_ass_tags(original, translated) -> bool` — extracts every `{...}`
  override-tag run from both strings with the regex `\{[^}]*\}` and returns
  `True` iff the **ordered list** of tags is identical
  (`original_tags == translated_tags`). Pure string logic, no ASS parser
  needed: empty `{}` matches; `{a{b}` matches the run `{a{b}` (first `}`
  closes); unbalanced trailing `{` after the last `}` matches nothing.
- The `ASS_TRANSLATION_PROMPT` constant (the tag-preservation prompt template,
  distinct from `TRANSLATION_PROMPT`), used verbatim as the prompt for ASS
  batches.
- The keep-original-on-mismatch branch inside `translate_ass_content`: for each
  translated event, if `validate_ass_tags(event.text, new_text)` is `True`
  write `new_text`, else **leave the original `event.text` unchanged** and log
  a warning. This deterministic fallback is what makes a tag-dropping LLM reply
  non-destructive; it is the load-bearing behavior, separate from the cue
  application itself.

This is the ASS-specific slice of translate-apply. The broader
`port-translate-srt-apply` covers SRT/VTT cue application and its falsifier only
names `*.in.srt`/`*.out.srt` (no ASS path, no tag-mismatch case). This item
isolates the tag-validation guard so it is proven independently.

## where
`rust/crates/submate-translate/src/lib.rs` — add `validate_ass_tags`, the
`ASS_TRANSLATION_PROMPT` const, and the keep-original branch in the
`translate_ass_content` path. `ASS_TRANSLATION_PROMPT` must be byte-for-byte
identical to the Python constant (the `{{...}}` doubling in the Python source is
`str.format` escaping for literal single braces — the emitted template contains
single braces `{\i1}`, `{\an8}`, etc.; only `{source_lang}`/`{target_lang}`/
`{text}` are real placeholders).

## why
Without the tag-preservation guard, a translation that drops or reorders
`{\pos(...)}`, `{\an8}`, `{\i1}` override tags would silently corrupt styling
and positioning in the output ASS file. Python's fallback keeps the original
text on any tag mismatch; the Rust port must match this exactly so a flaky LLM
reply degrades identically (original kept) rather than emitting broken ASS.

## falsifies
`cargo test -p submate-translate parity::ass_apply` transforms
`rust/fixtures/translate/ass_tags.in.ass` + `rust/fixtures/translate/ass_tags.mock_llm.json`
→ `rust/fixtures/translate/ass_tags.out.ass` exactly. The mock LLM must include
at least one reply with a **dropped/reordered** `{...}` tag so the
keep-original branch is exercised (that cue's output equals the original input
cue, not the LLM reply).

**requires fixture: rust/fixtures/translate/ass_tags.in.ass,
ass_tags.mock_llm.json, ass_tags.out.ass (capture first)** — capture by running
the Python `translate_ass_content` against a small ASS file whose mocked LLM
replies cover both a tag-preserving cue and a tag-dropping cue, dumping input,
mock map, and output. Fixtures are denylisted for grind agents; flag for a
human capture run under `rust/fixtures/capture/`.

In the interim (before the golden lands), `validate_ass_tags` is pure and
self-contained, so an inline `#[test]` table covering the regex edge cases
(`{}`, `{a{b}`, unbalanced trailing `{`, identical vs reordered tag lists)
proves the core comparison without the fixture.

---

**META note (round 3 re-triage, 2026-06-12):** unparked from `needs-human/` →
`backlog/`. `validate_ass_tags` is pure string/regex logic and the
`translate_ass_content` capture (mocked LLM, no external API/credential/network)
is pure-data — it fits the documented capture pre-pass rule in
`meta-contention.md`, not a human gate. Dependency `port-subtitle-ass-tags` was
unparked the same round. Next capture pre-pass should author the
`rust/fixtures/translate/ass_tags.*` golden (both a tag-preserving and a
tag-dropping cue) before dispatch. Do NOT re-park to `needs-human/`.
