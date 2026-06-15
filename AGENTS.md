# Repository Guidelines

**submate** is an AI subtitle tool — Whisper transcription (whisper.cpp via
`whisper-rs`) plus LLM translation. It is a native **Rust** Cargo workspace; it
ships a CLI (`transcribe` / `translate` / `probe` / `config`) and a server that
acts as a **Bazarr Whisper ASR provider**. Both transcribe in-process — there is
no queue or distributed-node system.

> History: submate began as a Python tool that was ported to Rust crate by crate;
> the Python implementation has been removed. The golden fixtures under
> `fixtures/` are now the workspace's own frozen snapshots (formerly captured
> from Python), used by the `parity::*` characterization tests.

## Repository structure

```
.
├── Cargo.toml            # virtual workspace (members under crates/)
├── crates/               # the Rust workspace
│   ├── submate-types     # shared enums (Device, WhisperModel, …)
│   ├── submate-lang      # the 100+ language table + ISO-639 conversions
│   ├── submate-config    # layered config (figment, SUBMATE__ env)
│   ├── stable-ts         # the stable-ts slice: regroup / suppress-silence / output
│   ├── submate-subtitle  # SRT/VTT parse+write, subtitle discovery
│   ├── submate-paths     # subtitle path building, Docker path mapping
│   ├── submate-media     # ffmpeg/ffprobe: track listing + audio extraction
│   ├── submate-whisper   # whisper.cpp inference + assembly + Dispatcher (model-gated)
│   ├── submate-translate # LLM backends (ollama/openai/gemini via async-openai, anthropic)
│   ├── submate-bazarr    # Bazarr glue: PCM↔f32, language-name table
│   ├── submate-server    # axum server: bazarr + ops routes
│   ├── submate-cli       # the `submate` binary (clap)
│   └── parity            # dev-only test helpers (golden loader, assert_*)
├── fixtures/             # frozen golden snapshots for parity tests
├── nix/                  # flake packaging (numtide/blueprint)
└── docs/                 # architecture.md + design notes
```

**Two seams matter.** *Pure-data* crates (`submate-types`, `-lang`, `-config`,
`stable-ts`, `-subtitle`, `-paths`, `parity`) carry **no** tokio/reqwest and have
exact byte-diff parity tests — keep them I/O-free. The **`model` feature**
(whisper.cpp) is confined to `submate-whisper` and `submate-cli`; the other
crates build and test **without** compiling whisper.cpp, which keeps the test
loop fast.

## Development environment

The flake (`nix develop`) provides the Rust toolchain, ffmpeg, and the
whisper.cpp build deps (clang/cmake/pkg-config). Run cargo from inside the
devshell.

| Command | Description |
|---|---|
| `nix develop` | dev shell with the toolchain + ffmpeg |
| `cargo build --workspace` | build (no model; fast) |
| `cargo build -p submate-cli --features model` | build the CLI with whisper.cpp |
| `nix build .#submate` | the `submate` CLI package (CPU) |
| `nix build .#submate-cuda` / `.#submate-vulkan` | GPU builds |
| `nix build .#docker-cpu` / `.#docker-gpu` / `.#docker-vulkan` | container images |
| `nix fmt` | format nix/shell/yaml/json/toml/markdown (Rust uses `cargo fmt`) |

## Build, test, lint

- **Rust**: edition **2024**, MSRV **1.90**. Format with `cargo fmt`.
- **The gate** (run from the repo root, inside the devshell):
  ```
  cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings
  ```
- **Lints**: `[workspace.lints]` enables a curated modernization set
  (`uninlined_format_args`, `manual_let_else`, `use_self`, `map_unwrap_or`, …) on
  top of `clippy::all`, all denied by the `-D warnings` gate. Prefer `#[expect]`
  over `#[allow]` (it self-cleans when the suppression is no longer needed).
- **Model-gated code** (`#[cfg(feature = "model")]`) is invisible to the default
  gate — verify it with `cargo clippy -p submate-cli --features model` and, for
  real behavior, a live run against a Whisper model.

## Architecture

Everything transcribes **in-process** (see `docs/architecture.md`). The shared
primitive is the **`Dispatcher`** in `submate-whisper` — a `Semaphore` +
`spawn_blocking` into whisper.cpp, sized to `server.concurrent_transcriptions` —
which both entry points run through:

- **`submate transcribe`** — extract the selected track's audio, transcribe via
  the `Dispatcher`, assemble, write the subtitle next to the input
  (`--translate-to` adds an LLM pass).
- **`submate server`** — the Bazarr Whisper provider. Bazarr's provider is
  synchronous, so `/bazarr/asr` and `/bazarr/detect-language` transcribe directly
  through the `BazarrTranscriber` seam in `submate-server` (production impl in
  `cmd_server`). Reliability contract: `/asr` returns the SRT in the body with a
  `Source` header on success and an **empty body on any failure** (the provider
  saves the body verbatim — an error envelope would become a corrupt subtitle);
  `/detect-language` always returns `200`.

## Configuration

All config uses the `SUBMATE__` prefix with `__` nesting (figment, mirroring the
old Pydantic schema). The CLI exposes per-run overrides (`--model`, `--language`,
`--format`, `--audio`, `--translate-to`, `--backend`, `--vad-model`, …); the rest
is set via `-c <config-file>` (`.env`/`.toml`/JSON) or env vars.

| Variable | Notes |
|---|---|
| `SUBMATE__WHISPER__MODEL` | **path to a GGML model** (`ggml-large-v3-turbo.bin`), not a name |
| `SUBMATE__WHISPER__VAD_MODEL` | path to a Silero VAD model → speech-only transcription |
| `SUBMATE__WHISPER__THREADS` | CPU thread override (default = whisper.cpp's `min(4, n_cpu)`) |
| `SUBMATE__SERVER__PORT` | default `9000` |
| `SUBMATE__TRANSLATION__BACKEND` | `ollama` (default) / `openai` / `claude` / `gemini` |

Note: GPU is selected at **build time** (the `cuda`/`vulkan`/… cargo features),
not by `SUBMATE__WHISPER__DEVICE`.

## Bazarr integration

Point Bazarr's Whisper provider at `http://<host>:9000/bazarr/asr`, enable
language detection, and set `SUBMATE__WHISPER__MODEL` to a GGML model path on the
server.

## Conventions

- Keep pure crates free of I/O deps; keep the `model` feature confined to the
  three crates above.
- `fixtures/` are golden truth — change them deliberately, not as a side effect.
- Adding a config field changes the serialized `Config` and the config parity
  fixtures together; keep them in lockstep.

## Commit guidelines

Conventional `{type}: {summary}` — `feat`, `fix`, `chore`, `refactor`, `test`,
`docs`, `style`. Describe the user-visible change. Before pushing, run the gate
(`cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings`)
and `nix flake check` for nix changes.
