# Port the Ollama backend

**blocked-by:** port-translate-backend-trait

## what
Port `OllamaBackend` over raw `reqwest` (POST /api/chat to ollama_url, parse message.content).

## where
`rust/crates/submate-translate/src/lib.rs`.

## why
The default local backend.

## falsifies
`cargo test -p submate-translate ollama_request_shape` asserts the request body matches the Python Ollama payload golden under `wiremock`.
