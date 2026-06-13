# Port `TranslationSettings.validate_for_target` (LLM-required guard)

## what
Port `TranslationSettings.validate_for_target(target_lang: str | None) -> None`
from `submate/config.py` (the method body shown at `def validate_for_target`).
It is the early "do we have a usable LLM backend for this target?" guard that
**both** the `translate` CLI command (`submate/cli/commands/translate.py`,
`config.translation.validate_for_target(target_lang)`) and
`TranscriptionService.transcribe_file` (`submate/queue/services/transcription.py`,
same call) run *before* doing any work. It is currently un-ported: `rg
validate_for_target rust/crates/` finds nothing.

The decision is pure data — `(backend, anthropic_api_key, openai_api_key,
gemini_api_key, target_lang) -> Ok | Err(message)` — with no IO and no whisper
dependency. Exact semantics to reproduce:

1. **No-op when no LLM is needed.** If `target_lang` is `None`/empty, OR
   `LanguageCode::from_string(target_lang) == LanguageCode::ENGLISH`, return
   `Ok(())` (English uses Whisper's built-in translate; no LLM). Note this routes
   through the already-ported `submate_lang` table, so `"en"`, `"eng"`,
   `"English"`, and case/whitespace variants all short-circuit.
2. **Backend match (LLM needed).**
   - `Ollama` → `Ok(())` always (no API key; "will fail at runtime if not
     running").
   - `Claude` → `Err` iff `anthropic_api_key` is empty.
   - `OpenAI` → `Err` iff `openai_api_key` is empty.
   - `Gemini` → `Err` iff `gemini_api_key` is empty.
3. **Error message is byte-exact** (Bazarr/CLI surface it to the user; it names
   the env var). For each backend the message is, with `{target}` =
   the raw `target_lang` string:
   ```
   Translation to '{target}' requires LLM. Set SUBMATE__TRANSLATION__ANTHROPIC_API_KEY or use SUBMATE__TRANSLATION__BACKEND=ollama
   ```
   substituting `ANTHROPIC_API_KEY` / `OPENAI_API_KEY` / `GEMINI_API_KEY`
   per backend. The two sentences are joined by a single space (Python
   concatenates the two `f"..."` string literals with no separator; the first
   ends with `". "`). Pin this exact string — a missing space or a renamed env
   var silently breaks the user-facing contract.

## where
`rust/crates/submate-config/src/lib.rs` as a method on the translation-settings
struct (it owns `backend` + the three `*_api_key` fields and already imports
`submate_types::TranslationBackend`). Add `submate_lang` as the dep for the
`ENGLISH` short-circuit if not already present. Return type:
`Result<(), String>` (or a small error type) carrying the exact message.

## why
This is the gate that decides "fail fast with a clear message" vs. "start a
transcription that will die at the LLM call". It is on the hot path of every
non-English `translate`/`transcribe --translate-to` invocation and its error
text is the user's only hint about which env var to set. Pure config logic =
exact-match parity layer per `rust/fixtures/README.md`.

## falsifies
`cargo test -p submate-config parity::validate_for_target` reproduces each
outcome against `rust/fixtures/config/validate_for_target_cases.json`: for a
table of `(backend, api_keys-present, target_lang)` rows it asserts the `Ok`
rows are `Ok(())` and the `Err` rows carry the byte-exact message string. Must
include: `target="en"`/`"eng"`/`"English"` short-circuit to `Ok` regardless of
backend/keys; `Ollama` always `Ok`; `Claude`/`OpenAI`/`Gemini` with empty key
→ the exact env-var message; with key present → `Ok`; `target_lang=None/""` →
`Ok`.

**requires fixture: `rust/fixtures/config/validate_for_target_cases.json`
(capture first).** It does not exist yet and the porter cannot write goldens
(`rust/fixtures/` denylisted). Add a capture entry to
`rust/fixtures/capture/capture_config.py` that drives the real
`TranslationSettings.validate_for_target` (it imports cleanly — pure data, no
external runtime) over the case table above, recording for each row whether it
raised and the exact `str(exc)` message. Until the golden lands the test must
self-skip (no-op pass), arming itself when the fixture appears.
