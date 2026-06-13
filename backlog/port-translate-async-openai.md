# port-translate: consolidate OpenAI/Ollama/Gemini onto async-openai

**blocked-by:** port-translate-async-trait

## what
Replace the three OpenAI-compatible backends (OpenAI, Ollama, Gemini) with a
single `OpenAiCompatBackend` built on the `async-openai` crate, configured by
`base_url`. Keep Anthropic as the native async `reqwest` backend (no official or
trustworthy Rust SDK — see the session discussion; raw async reqwest for the
Messages API is ~15 lines and avoids an unofficial dependency on the primary
backend).

- Add `async-openai` (workspace + `submate-translate`).
- `OpenAiCompatBackend { client: async_openai::Client<OpenAIConfig>, model }` —
  one `complete` that sends a chat-completion (single user message = the prompt)
  and returns the first choice's content.
- `make_backend` routing (the `TranslationBackend` enum + config stay the same,
  user-facing):
  - `Ollama` → `OpenAiCompatBackend` with `base_url = "{ollama_url}/v1"`, model `ollama_model`, no key (or a dummy).
  - `Openai` → `OpenAiCompatBackend` with the default OpenAI base, `openai_api_key`, `openai_model`.
  - `Gemini` → `OpenAiCompatBackend` with `base_url = "https://generativelanguage.googleapis.com/v1beta/openai"`, `gemini_api_key`, `gemini_model`.
  - `Claude` → the native async `AnthropicBackend`.
- Delete the old `OllamaBackend` / `OpenAIBackend` / `GeminiBackend` structs and
  their request/response types. `AnthropicBackend` and the chunking/apply layer
  stay.

**Verify-in-work:** Gemini's OpenAI-compat endpoint + key handling is the
youngest surface — confirm the `base_url`/path/model-name shape against
`async-openai` (a `base_url` ending in `/openai` so the crate appends
`/chat/completions`). If it can't be made to work cleanly, keep Gemini as a
native async reqwest backend and note it; do NOT silently ship a broken Gemini.

## where
- `rust/crates/submate-translate/src/lib.rs` — `OpenAiCompatBackend`, rewritten
  `make_backend`, delete the 3 old backends.
- `rust/Cargo.toml` + `submate-translate/Cargo.toml` — add `async-openai`.

## why
One mature library covers three providers (OpenAI + Ollama + Gemini) via
`base_url`, replacing three hand-maintained request/response shapes; only
Anthropic (no good Rust option) stays hand-rolled. Net: 4 hand-rolled backends →
1 `async-openai` + 1 native Anthropic.

## falsifies
`cargo test -p submate-translate` green, including:
- `backend_factory_routing`: each `TranslationBackend` variant builds the
  expected backend — `Ollama`/`Openai`/`Gemini` → an `OpenAiCompatBackend`
  whose configured `base_url` matches the table above; `Claude` → the Anthropic
  backend (assert via `.id()` and, for the compat ones, an exposed `base_url()`
  / a wiremock that the request lands on `…/chat/completions`).
- the chunking/apply tests still pass over an async mock `Backend`.

Real end-to-end calls (live OpenAI-compat + live Claude) are verified by a human
after merge — the offline gate cannot exercise the network.
