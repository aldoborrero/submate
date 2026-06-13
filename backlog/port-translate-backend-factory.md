# port-translate: shared backend factory (CLI + node)

## what
Extract the `config.translation.backend` → `Box<dyn Backend>` construction
(currently a private `build_backend` match in `submate-cli/src/main.rs`) into a
single reusable factory in `submate-translate`, so the node can build the exact
same backends without duplicating the match. Keep `submate-translate` free of a
`submate-config` dependency by passing the needed fields explicitly (a small
borrowed settings struct), not `&Config`.

Add:
- `pub struct BackendSettings<'a>` in `submate-translate` with the fields the
  four constructors need: `backend: submate_types::TranslationBackend`,
  `ollama_model`, `ollama_url`, `anthropic_api_key`, `claude_model`,
  `openai_api_key`, `openai_model`, `gemini_api_key`, `gemini_model` (all `&'a str`).
- `pub fn make_backend(s: &BackendSettings<'_>) -> Box<dyn Backend>` — the match
  moved verbatim from the CLI.
- `fn id(&self) -> &'static str` on the `Backend` trait, returning
  `"ollama"`/`"claude"`/`"openai"`/`"gemini"` (each impl returns its own name) —
  useful for logging which backend ran, and the observable the factory test
  asserts on.

Then rewrite the CLI's `build_backend` to build a `BackendSettings` from
`&config.translation` and delegate to `submate_translate::make_backend`.

## where
- `rust/crates/submate-translate/src/lib.rs` — `BackendSettings`, `make_backend`,
  `Backend::id` (+ the four impls). Add a `submate-types` dependency if not
  already present (for `TranslationBackend`).
- `rust/crates/submate-cli/src/main.rs` — `build_backend` delegates to the
  factory.

## why
The node needs to construct a translation backend from config to translate jobs
(port-translate-end-to-end); without a shared factory it would copy the CLI's
four-arm match, which then drifts. One factory, one source of truth.

## falsifies
`cargo test -p submate-translate` green, including `backend_factory_ids`: for
each `TranslationBackend` variant, `make_backend(&BackendSettings { backend: v, .. })`
returns a backend whose `.id()` equals the expected name
(`ollama`/`claude`/`openai`/`gemini`). Plus `submate-cli` still builds (its
`build_backend` delegates), proving the CLI path is unchanged.
