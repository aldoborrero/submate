# Parity: mocked-LLM ASS dialogue apply (tag-preservation gate) matches golden

**blocked-by:** port-translate-ass-tag-validate

## what
Add a golden-fixture parity falsifier for `submate_translate::translate_ass_dialogue`
(ports the tag-preservation body of `TranslationService.translate_ass_content`
in `submate/translation.py`).

The function is already implemented in
`rust/crates/submate-translate/src/lib.rs`: it translates pre-extracted ASS
dialogue lines in chunks (joined with `|||SUBTITLE_BREAK|||` under
`ASS_TRANSLATION_PROMPT`) and, **per line, keeps the translation only when
`validate_ass_tags` confirms the `{...}` override tags are unchanged**,
otherwise keeping the original (Python's "tag mismatch, keeping original"
fallback). Today this gate has only an inline unit test
(`translate_ass_dialogue` test with hand-written input); there is no golden
falsifier driving recorded LLM completions through the ASS prompt.

Scope note (intentional): the Rust workspace has no ASS (de)serializer, so this
ports the *portable core* — the line-level translate-and-gate over already
extracted dialogue `texts`, returning a vec aligned 1:1 with input. The full
`pysubs2.from_string` -> `to_string("ass")` round-trip is out of scope here and
belongs to a future ASS-serializer item (see `spike-ass-subparse-roundtrip.md`).
This item only guards the dialogue-line transform, which is the parity-critical
business logic.

Falsifier-driving details, all concrete:
- separator token is `|||SUBTITLE_BREAK|||`;
- prompt template is `ASS_TRANSLATION_PROMPT` (distinct from the SRT/VTT
  `TRANSLATION_PROMPT`), formatted via `format_prompt` — so the recorded prompt
  keys are unlike both `mock_llm.json` and the VTT map;
- `chunk_size` default is `50`;
- the golden MUST include at least one line whose recorded completion ALTERS a
  `{...}` tag, so the keep-original branch is exercised (not just the happy
  path), and at least one tagged line translated correctly to exercise the
  keep-translation branch.

## where
Test: new test in `rust/crates/submate-translate/tests/apply.rs` (or sibling
`ass_apply.rs`), mirroring the existing SRT `apply()` mocked-backend wiring
(exact-key `{prompt: completion}` lookup, no HTTP).

Requires fixtures: `rust/fixtures/translate/ass_dialogue.json` (the input
`texts` array + expected output `texts` array, the latter produced by the
Python tag-preservation logic so the keep-original outcomes are authoritative)
and `rust/fixtures/translate/mock_llm_ass.json` (the prompt/completion map for
the `ASS_TRANSLATION_PROMPT` batch). Capture first — scout cannot write under
`rust/fixtures/` (denylisted). Flag for human/capture.

## why
The ASS dialogue translate-and-gate is implemented but its parity to the Python
keep-original-on-tag-mismatch behavior is only spot-checked inline, not pinned
to a captured golden. The keep-original branch (the safety net that prevents
corrupting `{\pos(...)}`/`{\an8}` tags) is the highest-value line to falsify.

## falsifies
`cargo test -p submate-translate ass_apply` — `translate_ass_dialogue` over the
input `texts` in `rust/fixtures/translate/ass_dialogue.json`, with completions
served from `rust/fixtures/translate/mock_llm_ass.json` and `chunk_size = 50`,
returns a vec equal to that fixture's expected `texts`: lines with preserved
tags take the translation, lines whose completion altered a `{...}` tag keep
the original (`parity::assert_str_eq` per element).
