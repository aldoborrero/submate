# Port `TranslationService._init_backend` missing-API-key validation

## what
Port the construction-time API-key validation that `TranslationService._init_backend`
(`submate/translation.py`) performs and that the Rust factory `make_backend`
(`rust/crates/submate-translate/src/lib.rs`) currently **skips entirely**.

Python's `_init_backend` raises `ValueError` with an exact message before
returning a backend whenever the selected backend needs an API key that is
empty:

- `TranslationBackend.CLAUDE` and `anthropic_api_key` empty →
  `"TRANSLATION__ANTHROPIC_API_KEY required for Claude backend"`
- `TranslationBackend.OPENAI` and `openai_api_key` empty →
  `"TRANSLATION__OPENAI_API_KEY required for OpenAI backend"`
- `TranslationBackend.GEMINI` and `gemini_api_key` empty →
  `"TRANSLATION__GEMINI_API_KEY required for Gemini backend"`
- `TranslationBackend.OLLAMA` → never raises (no key; Ollama's OpenAI-compat
  surface ignores the key).

The Rust `make_backend` has signature `(&BackendSettings) -> Box<dyn Backend>`
— **infallible**. For Claude/OpenAI/Gemini with an empty key it silently
constructs a backend carrying a blank key, deferring the failure to a runtime
HTTP 401 instead of the immediate, descriptive `ValueError` Python emits at
construction. Change the factory (or add a sibling `try_make_backend`) to return
`Result<Box<dyn Backend>, E>`, where `E` carries the byte-exact message strings
above. Keep the existing infallible call sites compiling (the single caller is
`rust/crates/submate-cli/src/main.rs`; thread the `Result` through or have it
unwrap a validated settings struct).

This is NOT the same as `port-translate-validate-for-target.md`: that item ports
`TranslationSettings.validate_for_target` from `config.py` — a *target-aware*
pre-flight guard with the different message `"Translation to '{x}' requires LLM.
Set SUBMATE__..._API_KEY or use SUBMATE__TRANSLATION__BACKEND=ollama"`.
`_init_backend` is target-agnostic, runs at backend construction, and uses the
shorter `"TRANSLATION__<KEY> required for <Backend> backend"` form. Both guards
exist in Python and both must be ported; this item covers only the
construction-time one.

## where
- `rust/crates/submate-translate/src/lib.rs` — `make_backend` /
  `BackendSettings`; add the empty-key check + a `MissingApiKey`-style error
  variant carrying the exact message.
- `rust/crates/submate-cli/src/main.rs` — the lone `make_backend` call site;
  propagate the new `Result`.
- Python spec: `submate/translation.py`, `TranslationService._init_backend`.

## why
Backend selection from `config.translation.backend` is config dispatch — an
exact-match parity layer per `rust/fixtures/README.md` (enum `.value` strings and
config-derived messages must match byte-for-byte). Today the Rust factory
diverges: it constructs a doomed backend instead of failing fast with the
Python message, so an operator who forgets `SUBMATE__TRANSLATION__OPENAI_API_KEY`
sees a generic HTTP 401 at translate time rather than the exact config error
Python prints at startup.

## falsifies
`cargo test -p submate-translate parity::init_backend_key_validation` reproduces
each row of `rust/fixtures/translate/init_backend_cases.json` byte-for-byte:

- Claude/OpenAI/Gemini with empty key → `Err` whose rendered message equals the
  golden string for that backend.
- Claude/OpenAI/Gemini with a non-empty key → `Ok`, and the boxed backend's
  `id()` matches (`"claude"`/`"openai"`/`"gemini"`).
- Ollama with any key (incl. empty) → `Ok`, `id() == "ollama"`.

requires fixture: rust/fixtures/translate/init_backend_cases.json (capture
first — the three message strings are denylisted from this scout; capture them
from `submate/translation.py::_init_backend` by constructing a `Config` with
each `TRANSLATION__BACKEND` and an empty key and recording the raised
`ValueError` text, plus the success `id()` for non-empty keys).
