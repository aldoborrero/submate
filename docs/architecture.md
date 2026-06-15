# submate architecture

submate is two things sharing one transcription core:

1. a **Bazarr Whisper ASR provider** — an HTTP server (`submate server`), and
2. a **local CLI** — `submate transcribe` / `translate` / `probe` / `config`.

Both transcribe **in-process**. There is no queue, no broker, and no distributed
processing-node system — transcription runs directly on the box that invoked it.

```
   Bazarr ──HTTP──▶  submate server                   submate transcribe <file>
                     /bazarr/asr                       (extract audio → … → write .srt)
                     /bazarr/detect-language
                            │                                   │
                            └──────────────┬────────────────────┘
                                           ▼
                                  Dispatcher (Semaphore)
                                  spawn_blocking → whisper.cpp
                                           ▼
                            regroup → suppress-silence → SRT/VTT/ASS/…
                                           ▼
                              optional LLM translation (per backend)
```

## The Dispatcher

The shared concurrency primitive (`submate-whisper`): a `tokio::sync::Semaphore`
sized to `server.concurrent_transcriptions` plus `spawn_blocking` into
whisper.cpp. Every transcription — a Bazarr request or a CLI file — acquires a
permit, so at most `runners` clips decode at once and the rest wait. whisper.cpp
inference is blocking CPU/GPU work, so it always runs on a blocking thread,
keeping the async runtime responsive.

## Bazarr path (`submate server`)

Bazarr's Whisper provider is a **synchronous** RPC: it holds the connection per
file and reads the subtitle from the response body. So `/bazarr/asr` transcribes
**directly** through the `Dispatcher` (the `BazarrTranscriber` seam in
`submate-server`, implemented by `WhisperBazarrTranscriber` in the CLI) and
returns the subtitle inline. Reliability contract: `/asr` returns the SRT in the
body with a `Source` header on success and an **empty body on any failure** (the
provider saves the body verbatim — an error envelope would become a corrupt
subtitle); `/detect-language` always returns `200` (`{detected_language,
language_code}`, or `{Unknown, und}` on failure). The server also exposes ops
routes `/` and `/status`.

## CLI path (`submate transcribe`)

`submate transcribe <file>` runs the same pipeline in-process: extract the
selected audio track to PCM (`submate-media`), transcribe through the
`Dispatcher`, assemble the subtitle (regroup → suppress-silence → format), and
write it next to the input. `--translate-to` adds an LLM translation pass over
the rendered subtitle. A batch / `--recursive` run shares the `Dispatcher`'s
runner cap.

## Crate placement

| Concern | Crate |
|---|---|
| Shared enums (`Device`, `WhisperModel`, `OutputFormat`, …) | `submate-types` |
| Language table + ISO-639 conversions | `submate-lang` |
| Layered config (figment, `SUBMATE__` env) | `submate-config` |
| stable-ts slice (regroup / suppress-silence / output) | `stable-ts` |
| SRT/VTT/ASS parse+write, subtitle discovery | `submate-subtitle` |
| Subtitle path building, Docker path mapping | `submate-paths` |
| ffmpeg/ffprobe: track listing + audio extraction | `submate-media` |
| whisper.cpp inference + assembly + the `Dispatcher` (model-gated) | `submate-whisper` |
| LLM translation backends (ollama/openai/gemini/anthropic) | `submate-translate` |
| Bazarr glue: PCM↔f32, language-name table | `submate-bazarr` |
| axum server: bazarr + ops routes | `submate-server` |
| the `submate` binary (clap) | `submate-cli` |
| dev-only test helpers (golden loader, assert_*) | `parity` |

## The model feature

whisper.cpp (via `whisper-rs`) is confined to the `model` cargo feature in
`submate-whisper` and `submate-cli`. The default build compiles and tests
without it (and without `LIBCLANG_PATH`/`cmake`); the `Dispatcher`'s concurrency
logic is testable without a model, while real inference and the GPU backends
(`cuda`/`vulkan`/`metal`/`hipblas`/`intel-sycl`) are gated on `model`.

## Verification

Pure-data layers (config, language table, paths, stable-ts, subtitle formatting,
mocked-LLM translation) are pinned by the golden fixtures under `fixtures/`. The
server's route shapes and the Bazarr provider contract (raw-PCM in, SRT-in-body,
empty-body-on-failure, `200`-`Unknown` on detect failure) are verified
behaviorally with integration tests.
