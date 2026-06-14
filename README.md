# submate

AI subtitle generation in native Rust — **Whisper** transcription (whisper.cpp
via `whisper-rs`) with optional **LLM translation**. Use it from the command
line, or run it as a **Bazarr Whisper ASR provider** so your media library gets
machine-generated subtitles on demand.

## Features

- **Transcribe** video/audio to `srt` / `vtt` / `ass` / `json` / `txt`, with
  word-level timestamps and silence suppression (stable-ts-style post-processing).
- **Translate** subtitles to any language via an LLM backend
  (Ollama, OpenAI, Anthropic, Gemini).
- **Bazarr integration** — drop-in Whisper ASR provider (`/bazarr/asr` +
  language detection).
- **GPU acceleration** — CUDA, Vulkan (incl. Intel iGPU), Metal, ROCm, Intel
  oneAPI.
- **Silero VAD** — transcribe only detected speech, cutting hallucinated lines
  over silence/music.
- Audio-track selection, Docker path mapping, multi-language audio handling.

## Install

The toolchain and runtime deps (ffmpeg, whisper.cpp build deps) live in the nix
flake:

```sh
nix build .#submate          # CPU build  → ./result/bin/submate
nix build .#submate-cuda     # NVIDIA (CUDA)
nix build .#submate-vulkan   # cross-vendor GPU (incl. Intel iGPU)
nix build .#docker-cpu       # container image (submate:cpu)
nix build .#docker-gpu       # container image (submate:gpu, needs nvidia-container-toolkit)
nix build .#docker-vulkan    # container image (submate:vulkan, pass --device /dev/dri + host ICD)
```

Or develop with `nix develop` and `cargo build -p submate-cli --features model`.

You also need a GGML Whisper model (e.g. from
[ggerganov/whisper.cpp](https://huggingface.co/ggerganov/whisper.cpp)).
**`large-v3-turbo`** is a good default (near-`large-v3` accuracy, much faster);
use `large-v3` for the best quality on hard audio or CJK.

## Quick start

```sh
# Transcribe a file in one shot (writes movie.srt next to it)
SUBMATE__WHISPER__MODEL=/models/ggml-large-v3-turbo.bin \
  submate transcribe movie.mkv --sync

# …choosing the audio track, output format, and a Silero VAD model
submate transcribe movie.mkv --sync \
  --audio lang:ja --format srt --vad-model /models/ggml-silero-v5.1.2.bin

# Translate an existing subtitle with an LLM
submate translate movie.ja.srt --target-lang en --backend claude

# Run the server (Bazarr ASR provider, embedded processing node)
SUBMATE__WHISPER__MODEL=/models/ggml-large-v3-turbo.bin submate server
```

`submate probe <file>` lists audio tracks; `submate config show` prints the
resolved configuration; `submate --help` lists everything.

## Bazarr

Point Bazarr's Whisper provider at `http://<host>:9000/bazarr/asr`, enable
language detection, and set `SUBMATE__WHISPER__MODEL` on the server. Bazarr calls
synchronously and submate transcribes on demand (sharing one concurrency limit
across requests).

## Configuration

Everything is configurable via the `SUBMATE__` env prefix (`__` for nesting) or a
config file (`-c config.toml`/`.env`/JSON). Common knobs:

| Variable | Notes |
|---|---|
| `SUBMATE__WHISPER__MODEL` | **path** to a GGML model (not a name) |
| `SUBMATE__WHISPER__VAD_MODEL` | path to a Silero VAD model → speech-only transcription |
| `SUBMATE__WHISPER__THREADS` | CPU thread override (default `min(4, n_cpu)`; more can *regress* small models) |
| `SUBMATE__SERVER__PORT` | default `9000` |
| `SUBMATE__TRANSLATION__BACKEND` | `ollama` (default) / `openai` / `claude` / `gemini` |
| `SUBMATE__TRANSLATION__<X>_API_KEY` | per-backend API key |

The CLI also exposes per-run overrides: `--model`, `--language`, `--format`,
`--audio`, `--translate-to`, `--backend`, `--vad-model`, `--sync`, …

## GPU

GPU offload is selected at **build time** by the cargo feature matching your host
(each implies the `model` feature and needs that backend's toolchain):

```sh
cargo build -p submate-cli --release --features cuda        # NVIDIA
cargo build -p submate-cli --release --features vulkan      # cross-vendor (incl. Intel iGPU)
cargo build -p submate-cli --release --features metal       # Apple Silicon
cargo build -p submate-cli --release --features hipblas     # AMD ROCm
cargo build -p submate-cli --release --features intel-sycl  # Intel oneAPI
```

A GPU-built binary uses the GPU automatically (no runtime flag).

## Architecture

A broker-less server + processing-node design (FileFlows/Unmanic-style): the
server owns media I/O and a durable SQLite queue and ships extracted audio to
nodes that pull work; a single box runs an embedded node by default. Bazarr is
served by a direct, synchronous transcription path rather than the queue. See
[docs/architecture.md](docs/architecture.md).

## Development

See [AGENTS.md](AGENTS.md) for the workspace layout and conventions. The check
gate (from a `nix develop` shell):

```sh
cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings
```

## License

submate is released under the MIT License — see [LICENSE](LICENSE).
