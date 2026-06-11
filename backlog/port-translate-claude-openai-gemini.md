# Port Claude / OpenAI / Gemini backends

**blocked-by:** port-translate-backend-trait

## what
Port the three cloud backends over raw `reqwest` (per-API request/response shapes, API-key headers, model defaults).

## where
`rust/crates/submate-translate/src/lib.rs`.

## why
Cloud translation options; each is a thin HTTP backend behind the trait.

## falsifies
`cargo test -p submate-translate parity::backend_payloads` matches each provider's request golden under `wiremock`.
