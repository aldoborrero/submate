# submate (Rust port)

A native-Rust port of submate, driven by the **grind** (autonomous multi-agent
loop — see `.claude/grind.config.js` and `backlog/`). The Python tree under
`submate/` is the executable spec and stays in place until the port reaches
parity.

## Why Rust, and the one hard part

~80% of submate (CLI, config, server, queue, translation, integrations) ports
mechanically. The hard part — Whisper inference + `stable-ts` post-processing —
collapses to a bounded ~1240-LOC reimplementation, because **word timestamps
come from the inference engine, not from stable-ts**. `whisper-rs` (whisper.cpp
bindings) supplies them, so stable-ts's heavy DTW alignment is never needed.

## Port map

| Python | Rust crate | Status |
|---|---|---|
| `types.py` | `submate-types` | stub |
| `language.py` | `submate-lang` | stub |
| `config.py` | `submate-config` | stub |
| — (server↔node wire types) | `submate-proto` | stub |
| stable-ts slice | `stable-ts` (model/regroup/suppress_silence/output) | stub |
| `subtitle.py` | `submate-subtitle` | stub |
| `paths.py` | `submate-paths` | stub |
| `media.py` | `submate-media` | stub |
| `whisper.py` | `submate-whisper` | stub |
| — (node agent + Dispatcher) | `submate-node` | stub |
| `translation.py` | `submate-translate` | stub |
| `media_servers/jellyfin.py` | `submate-jellyfin` | stub |
| `bazarr/` | `submate-bazarr` | stub |
| `queue/` | `submate-queue` | stub |
| `server/` | `submate-server` | stub |
| `cli/` | `submate-cli` (bin `submate`) | stub |
| — | `parity` (test helpers) | **done** |

The pure-data crates (`submate-types`, `-lang`, `-config`, `submate-proto`,
`stable-ts`, `-subtitle`, `-paths`, `parity`) stay free of tokio/reqwest/rusqlite
so they remain exact-diff testable against golden fixtures.

## Topology

submate-rs is a **server + processing-node** system (FileFlows/Unmanic-style,
no broker): a central server runs where the media is, owns the durable queue and
all ffmpeg I/O, and ships extracted PCM to processing nodes that pull work. See
[docs/architecture.md](docs/architecture.md). The coordination layer is a new
design (not a port of Python's single-box queue) and is verified behaviorally;
the business logic it carries (skip conditions, output formatting) keeps
parity-against-Python falsifiers.

## Build & test

The toolchain (cargo, clippy, clang for whisper-rs) lives in the nix devshell:

```sh
nix develop --command bash -c '
  cargo test  --manifest-path rust/Cargo.toml --workspace
  cargo clippy --manifest-path rust/Cargo.toml --workspace --all-targets -- -D warnings
'
```

That command is the grind's `fastCheck` — every backlog item is "done" iff its
named `parity::*` test passes under it.

## GPU acceleration

Transcription runs on CPU by default. To build with whisper.cpp GPU offload,
enable the cargo feature matching the host GPU (each implies `model` and needs
that backend's toolchain at build time):

```sh
cargo build --manifest-path rust/Cargo.toml -p submate-cli --release --features cuda     # NVIDIA (CUDA toolkit + nvcc)
cargo build … --features metal                                                           # Apple Silicon
cargo build … --features hipblas                                                         # AMD ROCm
cargo build … --features vulkan                                                          # cross-vendor (incl. Intel iGPU)
cargo build … --features intel-sycl                                                      # Intel oneAPI
```

A GPU feature flips whisper-rs's `use_gpu` on automatically (it defaults to
`cfg!(feature = "_gpu")`), so a binary built with `cuda` uses the GPU with no
runtime flag.

`SUBMATE_WHISPER_THREADS` overrides the CPU inference thread count (default =
whisper.cpp's `min(4, n_cpu)`; raising it can *regress* small models, which are
memory-bandwidth-bound — measure per host).

`SUBMATE_WHISPER_VAD_MODEL=<path>` enables Silero **VAD**: transcribe only
detected speech, skipping silence/non-speech. This removes whisper's
hallucinated lines over silence/music and speeds up sparse audio (timestamps are
mapped back to the original clip). Point it at a `ggml-silero-*.bin` model. (We
drive VAD via the standalone `WhisperVadContext` because whisper-rs's `full`
bypasses whisper.cpp's built-in `--vad`.)

> Note: the Rust port is not nix-packaged yet, so `nix build .#docker-gpu` still
> builds the **Python** image. A CUDA-enabled Rust image is a follow-up to
> packaging the `rust/` workspace in nix.

## Running the port

1. Land this scaffold on `origin/main`.
1. Capture golden fixtures once: run `rust/fixtures/capture/*.py` against the
   Python tree (see `rust/fixtures/capture/README.md`). Commit `rust/fixtures/`.
1. Launch `/grind` (3 implementers + a rotating port specialist) and let it work
   the `backlog/` until dry.

See `rust/fixtures/README.md` for the parity contract.
