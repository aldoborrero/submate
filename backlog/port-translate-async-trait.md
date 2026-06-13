# port-translate: make the translation stack async

## what
Convert the translation layer from blocking to async, so backends `.await`
naturally inside the async node and the blocking workarounds can be deleted.
(Prerequisite for adopting `async-openai`, which is async-only.)

- `Backend` trait → async: `#[async_trait::async_trait] ... async fn complete(&self, prompt: &str) -> Result<String, BackendError>`
  (the `async-trait` crate keeps it object-safe for `Box<dyn Backend>`). Keep
  `fn id(&self) -> &'static str`.
- The four existing backends (Ollama/Claude/OpenAI/Gemini) → async `reqwest::Client`
  (`.send().await`). Delete the per-call `http_client()` helper and the stored-
  vs-per-call dance — async reqwest has no internal-runtime problem.
- `translate_srt_content` / `translate_vtt_content` / `translate_ass_dialogue`
  (and `translate_ass_content`) → async, awaiting `complete`. Their `complete`
  callback becomes an async closure / they take `&impl Backend`.
- `submate-node`: `TranslationStep::translate` → async, awaiting the backend.
  In `Agent::run_job`, replace the `spawn_blocking` + `Arc`/clone shuffle (added
  only to host blocking reqwest) with a direct `.await`. `TranslationStep` can
  hold the backend without the `Arc`-for-spawn_blocking requirement.
- `submate-cli`: the standalone `cmd_translate` is sync, so wrap the async
  translate in a `tokio` runtime (`Runtime::new()?.block_on(...)`), mirroring
  `cmd_transcribe`. The `--sync` embedded-node path is already async.
- Update tests: the mock backend becomes an async `Backend` impl.

This keeps all four backends (now async) — the `async-openai` consolidation is a
separate follow-up (`port-translate-async-openai`).

## where
- `rust/crates/submate-translate/src/lib.rs` — trait, 4 backends, apply fns,
  remove `http_client`.
- `rust/crates/submate-node/src/lib.rs` — `TranslationStep`, `run_job` (drop the
  `spawn_blocking`/`Arc` translation shuffle).
- `rust/crates/submate-cli/src/main.rs` — `cmd_translate` runtime.
- Add `async-trait` to the workspace deps + `submate-translate`.

## why
Going async is the prerequisite for `async-openai` and dissolves the
blocking-reqwest-in-async problem at the root (the workaround in commit
`1f3fa16` becomes unnecessary) instead of routing around it.

## falsifies
`cargo test -p submate-translate -p submate-node` green under the async trait,
including: `translate_srt_content` (async) drives an **async mock `Backend`**
(deterministic transform) over `sampleA.in.srt` → `sampleA.out.srt`; the
chunking/apply tests pass unchanged in content; and the node
`translate_post_step` test awaits an async stub backend. `cargo clippy
--all-targets -D warnings` clean (no leftover `spawn_blocking`/`http_client`).
