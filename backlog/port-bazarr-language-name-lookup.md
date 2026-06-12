# Port BazarrService language-code → display-name lookup

**blocked-by:** none (pure-data table + two-step lookup; takes a detected
language *code* string as input, so it does NOT depend on whisper. It is a
leaf dependency *of* the whisper-blocked `port-queue-bazarr-service` detect
path, not the other way around.)

## what
Port the `BazarrService.LANGUAGE_NAMES` table and the two-step
detect-language naming logic from `submate/queue/services/bazarr.py` as a
pure function. This is the exact computation that turns Whisper's detected
language code into the `{"detected_language", "language_code"}` pair that the
Bazarr detect-language endpoint returns.

The logic in `BazarrService.detect_language` (post-transcription, code only —
drop the whisper call itself, which belongs to `port-queue-bazarr-service`):

1. `language_code = <whisper detected language> or "und"`
   (Python `result.language or "und"` — empty string / None / missing → `"und"`).
2. `language_name = LANGUAGE_NAMES.get(language_code, "Unknown")`.
3. Return `{"detected_language": language_name, "language_code": language_code}`.

**The table is a deliberately NARROW, hardcoded 29-entry map, NOT the full
`submate-lang` `name_en` table.** This is the parity trap:

- Exactly these 29 codes get a name:
  en→English, es→Spanish, fr→French, de→German, it→Italian, pt→Portuguese,
  ru→Russian, ja→Japanese, zh→Chinese, ko→Korean, ar→Arabic, hi→Hindi,
  nl→Dutch, pl→Polish, tr→Turkish, vi→Vietnamese, th→Thai, sv→Swedish,
  da→Danish, fi→Finnish, no→Norwegian, cs→Czech, el→Greek, he→Hebrew,
  hu→Hungarian, id→Indonesian, ms→Malay, ro→Romanian, sk→Slovak, uk→Ukrainian.
- **Any code outside the set → `"Unknown"`**, even valid ISO-639-1 codes the
  full `submate-lang` table *would* name (e.g. `"ca"` Catalan, `"uk"` is in
  the set but `"be"` Belarusian is not, `"fa"` Persian is not). Do NOT route
  this through `submate-lang::name_en`; that would name codes the Python map
  leaves as `"Unknown"` and silently diverge the Bazarr response.
- `"und"` itself is not a key → name `"Unknown"`. So a no-detection result is
  `{"detected_language": "Unknown", "language_code": "und"}` — which is also
  exactly the `detect_language_task` error-envelope default (see
  `port-queue-models-results`); keep the two in sync but source both from this
  one table/fallback so they cannot drift.

Signature suggestion (a const lookup + a free fn):
`pub fn detect_language_name(code: &str) -> &'static str` returning the mapped
name or `"Unknown"`, plus `pub fn normalize_detected_code(whisper_lang:
Option<&str>) -> String` applying the `or "und"` rule (treating `Some("")` as
absent, matching Python truthiness).

## where
`rust/crates/submate-bazarr/src/lib.rs` (currently a 3-line stub; it will also
host `wrap_pcm_as_wav` from `port-bazarr-pcm-wav-wrap`). Keep this free of
whisper/queue/HTTP deps so it stays one-worktree-sized and is reusable by both
the queue bazarr-service detect path and the detect-language error envelope.

## why
The 29-entry table and the `"Unknown"`/`"und"` fallbacks are the Bazarr
detect-language wire contract. Bazarr displays `detected_language` to the user
and keys off `language_code`; using the broader `submate-lang` names would name
languages Python leaves as `"Unknown"`, drifting the response for every
out-of-set detection. Pure-data layer ⇒ byte-for-byte parity.

## falsifies
`cargo test -p submate-bazarr parity::language_name_lookup` asserts, exactly
against the golden, the `detected_language`/`language_code` pair for:

- every one of the 29 in-set codes → its mapped name,
- several out-of-set-but-valid-ISO codes (e.g. `ca`, `fa`, `be`) → `"Unknown"`,
- the absent-detection cases (`None`, `Some("")`) → code `"und"`, name `"Unknown"`,
- a bogus code (e.g. `xx`) → code `"xx"`, name `"Unknown"`.

**requires fixture: `rust/fixtures/queue/bazarr_language_names.json` (capture
first).** No golden for this table exists yet (`rust/fixtures/queue/` has only
`enum_values.json`) and the porter cannot write goldens (denylisted). This
capture is pure-data with NO external runtime — `submate.queue.services.bazarr`
imports cleanly (the table is a class attribute; no whisper load needed to read
`BazarrService.LANGUAGE_NAMES`). Per the triage rule in `meta-contention.md`
(pure-data captures run in the capture pre-pass, item stays in `backlog/`), the
next capture pre-pass should author e.g.
`rust/fixtures/capture/capture_bazarr_lang.py` that emits, for an input list of
codes (the 29 in-set + the out-of-set/absent/bogus cases above), the
`{detected_language, language_code}` Python computes via `LANGUAGE_NAMES.get(...)`
and the `or "und"` rule — driving the real attribute/lookup so the table stays
Python-sourced. Land the golden in a deliberate capture commit before dispatch;
falsifier blocked until it lands. Do NOT re-park to `needs-human/`.

---

**META capture pre-pass (round 3, 2026-06-12):** golden landed —
`rust/fixtures/queue/bazarr_language_names.json`, captured by
`rust/fixtures/capture/capture_bazarr_lang.py` driving the live
`BazarrService.LANGUAGE_NAMES` table + the `or "und"` / `.get(...,"Unknown")`
logic in the nix devshell. The item is now a pure port with its oracle present;
no fixture work left for the porter.

Spec correction surfaced by the capture: the table has **30** entries, not 29 —
the prose "29-entry" miscounts the very list it enumerates (all 30 codes
en..uk are present in `submate/queue/services/bazarr.py`). Port against the
golden (the authoritative shape), which carries the full `language_names` map
plus 36 `cases` (30 in-set + ca/fa/be/xx out-of-set + None/"" absent).
