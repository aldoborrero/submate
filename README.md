# Submate

AI-powered subtitle generation tool using Whisper with LLM translation capabilities.

## Features

- **Whisper Transcription**: Generate subtitles from audio/video files using OpenAI Whisper (via faster-whisper and stable-ts)
- **LLM Translation**: Translate subtitles using multiple backends (Ollama, OpenAI, Claude, Gemini)
- **Media Server Integration**: Webhooks for Bazarr and Jellyfin
- **Task Queue**: Async processing with Huey for handling multiple transcription jobs
- **CLI & Server**: Both command-line tools and HTTP API available

## Installation

### Using Nix (Recommended)

```bash
# Run directly
nix run github:aldoborrero/submate -- --help

# Or enter development shell
nix develop
```

### Using pip

```bash
pip install submate

# With LLM backends
pip install submate[ollama]      # Ollama (local, free)
pip install submate[openai]      # OpenAI
pip install submate[claude]      # Anthropic Claude
pip install submate[gemini]      # Google Gemini
pip install submate[all-llm]     # All backends
```

### Using Docker

```bash
# CPU version
docker run -p 9000:9000 -v /media:/data ghcr.io/aldoborrero/submate:cpu

# GPU version (requires nvidia-container-toolkit)
docker run --gpus all -p 9000:9000 -v /media:/data ghcr.io/aldoborrero/submate:gpu
```

## Quick Start

### Transcribe a Video

```bash
# Basic transcription
submate transcribe movie.mkv

# With translation to Spanish
submate transcribe movie.mkv --translate-to es

# Select Japanese audio track
submate transcribe movie.mkv --audio-language ja

# Process entire directory
submate transcribe ./movies/ -r
```

### Translate Existing Subtitles

```bash
# Translate SRT file
submate translate movie.en.srt -t es

# With explicit source language
submate translate subtitles.srt -s ja -t en -o subtitles_english.srt

# Process directory recursively
submate translate ./subs/ -t fr -r
```

### Start the Server

```bash
# Start server (default port 9000)
submate server

# Start background worker
submate worker
```

## Configuration

All configuration uses environment variables with `SUBMATE__` prefix:

### Whisper Settings

| Variable | Description | Default |
|----------|-------------|---------|
| `SUBMATE__WHISPER__MODEL` | Model size (tiny, base, small, medium, large) | `medium` |
| `SUBMATE__WHISPER__DEVICE` | Device (cpu, cuda, auto) | `cpu` |
| `SUBMATE__WHISPER__COMPUTE_TYPE` | Precision (int8, float16, float32) | `int8` |
| `SUBMATE__WHISPER__LANGUAGE` | Force source language | auto-detect |

### Translation Settings

| Variable | Description | Default |
|----------|-------------|---------|
| `SUBMATE__TRANSLATION__BACKEND` | LLM backend (ollama, openai, claude, gemini) | `ollama` |
| `SUBMATE__TRANSLATION__OLLAMA_MODEL` | Ollama model name | `llama3.2` |
| `SUBMATE__TRANSLATION__OPENAI_API_KEY` | OpenAI API key | - |
| `SUBMATE__TRANSLATION__ANTHROPIC_API_KEY` | Anthropic API key | - |
| `SUBMATE__TRANSLATION__GEMINI_API_KEY` | Google Gemini API key | - |

### Server Settings

| Variable | Description | Default |
|----------|-------------|---------|
| `SUBMATE__SERVER__HOST` | Server host | `0.0.0.0` |
| `SUBMATE__SERVER__PORT` | Server port | `9000` |
| `SUBMATE__SERVER__BAZARR_ENABLED` | Enable Bazarr integration | `true` |
| `SUBMATE__SERVER__JELLYFIN_ENABLED` | Enable Jellyfin integration | `true` |

### Jellyfin Settings

| Variable | Description | Default |
|----------|-------------|---------|
| `SUBMATE__JELLYFIN__SERVER_URL` | Jellyfin server URL | - |
| `SUBMATE__JELLYFIN__API_KEY` | Jellyfin API key | - |

## API Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/` | GET | Server info and health |
| `/status` | GET | Server status with queue stats |
| `/queue` | GET | Queue statistics |
| `/bazarr/asr` | POST | Bazarr ASR transcription |
| `/bazarr/detect-language` | POST | Bazarr language detection |
| `/webhooks/jellyfin` | POST | Jellyfin event webhook |

## Integrations

### Bazarr

Submate acts as a Whisper ASR provider for [Bazarr](https://bazarr.media/):

1. In Bazarr, go to Settings → Subtitles → Whisper Provider
1. Set endpoint: `http://submate:9000/bazarr/asr`
1. Enable language detection: `http://submate:9000/bazarr/detect-language`

### Jellyfin

Webhook-based integration for automatic transcription:

1. Install the [Jellyfin Webhook plugin](https://github.com/jellyfin/jellyfin-plugin-webhook)
1. Add webhook URL: `http://submate:9000/webhooks/jellyfin`
1. Enable ItemAdded events for Movies/Episodes

## Docker Compose Example

```yaml
services:
  submate:
    image: ghcr.io/aldoborrero/submate:cpu
    ports:
      - "9000:9000"
    environment:
      - SUBMATE__WHISPER__MODEL=medium
      - SUBMATE__TRANSLATION__BACKEND=ollama
      - SUBMATE__JELLYFIN__SERVER_URL=http://jellyfin:8096
      - SUBMATE__JELLYFIN__API_KEY=your-api-key
    volumes:
      - /media:/data
      - whisper-models:/root/.cache/huggingface

  submate-worker:
    image: ghcr.io/aldoborrero/submate:cpu
    command: worker
    environment:
      - SUBMATE__WHISPER__MODEL=medium
    volumes:
      - /media:/data
      - whisper-models:/root/.cache/huggingface

volumes:
  whisper-models:
```

## Development

### Setup

```bash
# Enter development shell (includes all tools)
nix develop

# Or install dev dependencies
pip install -e ".[dev]"
```

### Running Tests

```bash
pytest tests/ -v
```

### Code Quality

```bash
# Format
ruff format submate/ tests/

# Lint
ruff check submate/ tests/

# Type check
mypy submate/ --ignore-missing-imports
```

### Building

```bash
# Build application
nix build .#submate

# Build Docker images
nix build .#docker-cpu
nix build .#docker-gpu

# Load into Docker
docker load < result
```

## License

MIT
