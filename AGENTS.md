# Repository Guidelines

This repository contains **submate**, an AI-powered subtitle generation tool using Whisper with LLM translation capabilities.

## Repository Overview

```
submate/
├── submate/                    # Python source code
│   ├── cli/                    # Click CLI commands
│   │   └── commands/           # Individual commands (transcribe, translate, etc.)
│   ├── queue/                  # Huey task queue system
│   │   ├── tasks/              # Task definitions
│   │   └── services/           # Business logic services
│   ├── server/                 # FastAPI server
│   │   └── handlers/           # Route handlers (bazarr, jellyfin, core)
│   ├── media_servers/          # Media server integrations (Jellyfin)
│   ├── config.py               # Pydantic settings configuration
│   ├── whisper.py              # Whisper/stable-ts integration
│   ├── translation.py          # LLM translation backends
│   └── ...
├── tests/                      # pytest test suite
├── nix/                        # Nix packaging
│   ├── packages/               # Package definitions
│   │   ├── submate/            # Main application package
│   │   ├── docker-cpu/         # CPU Docker image
│   │   ├── docker-gpu/         # GPU Docker image
│   │   └── stable-ts/          # Custom stable-ts package
│   └── devshell.nix            # Development shell
└── pyproject.toml              # Python project configuration
```

## Development Environment Setup

### Installing Nix

Install Nix if not available:

```bash
# Install Nix with daemon mode
sh <(curl -L https://nixos.org/nix/install) --daemon

# Enable flakes and nix-command
echo "experimental-features = nix-command flakes" | sudo tee -a /etc/nix/nix.conf

# Restart the Nix daemon
sudo systemctl restart nix-daemon  # Linux
```

### Development Shell

Enter the development shell for full tooling:

```bash
nix develop
```

### Essential Commands

| Command | Description |
|---------|-------------|
| `nix develop` | Enter dev shell with all tools |
| `nix build .#submate` | Build the application |
| `nix build .#docker-cpu` | Build CPU Docker image |
| `nix build .#docker-gpu` | Build GPU Docker image |
| `nix flake check` | Run all checks |
| `nix fmt` | Format all files |

______________________________________________________________________

## Python Development

### Code Style

- **Python Version**: 3.13+
- **Formatter**: `ruff format`
- **Linter**: `ruff check`
- **Type Checker**: `mypy`
- **Indentation**: 4 spaces (Python), 2 spaces (Nix)

### Running Checks

```bash
# Format code
ruff format submate/ tests/

# Lint code
ruff check submate/ tests/

# Type check
mypy submate/ --ignore-missing-imports

# Run tests
pytest tests/ -v

# All checks
ruff format submate/ tests/ && ruff check submate/ tests/ && mypy submate/ --ignore-missing-imports && pytest tests/
```

### Project Structure Conventions

| Directory | Purpose |
|-----------|---------|
| `submate/cli/commands/` | One file per CLI command |
| `submate/queue/tasks/` | Huey task definitions |
| `submate/queue/services/` | Business logic (transcription, bazarr) |
| `submate/server/handlers/` | FastAPI route handlers by integration |

### Adding a New CLI Command

1. Create `submate/cli/commands/{command}.py`:

```python
"""Command description."""

import click

from submate.cli.utils import console, setup_logging
from submate.config import get_config


@click.command()
@click.argument("path", type=click.Path(exists=True))
@click.option("--option", "-o", help="Option description")
@click.pass_context
def mycommand(ctx: click.Context, path: str, option: str | None) -> None:
    """Command help text."""
    config = get_config(ctx.obj.get("config_file"))
    # Implementation
```

2. Register in `submate/cli/commands/__init__.py`
1. Add to `submate/cli/main.py`

### Adding a New Queue Task

1. Create task in `submate/queue/tasks/{task}.py`:

```python
from typing import Any

from submate.config import Config
from submate.queue.models import TaskResult
from .base import BaseTask


class MyTask(BaseTask[ResultType]):
    """Task description."""

    service: MyService

    @property
    def task_name(self) -> str:
        return "my_task"

    def __init__(self, config: Config, my_service: MyService, **kwargs: Any) -> None:
        super().__init__(config, my_service=my_service, **kwargs)
        self.service = my_service

    def execute(self, **kwargs: Any) -> TaskResult[ResultType]:
        # Implementation
        return TaskResult(success=True, data=result)
```

2. Export in `submate/queue/tasks/__init__.py`

______________________________________________________________________

## Configuration

### Environment Variables

All configuration uses `SUBMATE__` prefix with nested `__` delimiter:

| Variable | Description | Default |
|----------|-------------|---------|
| `SUBMATE__WHISPER__MODEL` | Whisper model size | `medium` |
| `SUBMATE__WHISPER__DEVICE` | Device (cpu, cuda, auto) | `cpu` |
| `SUBMATE__SERVER__PORT` | Server port | `9000` |
| `SUBMATE__TRANSLATION__BACKEND` | LLM backend | `ollama` |
| `SUBMATE__TRANSLATION__OPENAI_API_KEY` | OpenAI API key | - |
| `SUBMATE__TRANSLATION__ANTHROPIC_API_KEY` | Anthropic API key | - |
| `SUBMATE__JELLYFIN__SERVER_URL` | Jellyfin server URL | - |
| `SUBMATE__JELLYFIN__API_KEY` | Jellyfin API key | - |

### Configuration Classes

Configuration is defined in `submate/config.py` using Pydantic Settings:

```python
class WhisperSettings(BaseModel):
    model: str = "medium"
    device: Device = Device.CPU
    # ...

class Config(BaseSettings):
    model_config = SettingsConfigDict(env_prefix="SUBMATE__")
    whisper: WhisperSettings = Field(default_factory=WhisperSettings)
    # ...
```

______________________________________________________________________

## Server & Integrations

### FastAPI Server

Server runs on port 9000 by default with these endpoints:

| Endpoint | Method | Purpose |
|----------|--------|---------|
| `/` | GET | Health check / info |
| `/status` | GET | Queue status |
| `/queue` | GET | Queue statistics |
| `/bazarr/asr` | POST | Bazarr ASR transcription |
| `/bazarr/detect-language` | POST | Bazarr language detection |
| `/jellyfin/webhook` | POST | Jellyfin event webhook |

### Bazarr Integration

Submate acts as a Whisper ASR provider for Bazarr:

1. Configure Bazarr: Settings → Subtitles → Whisper Provider
1. Set endpoint: `http://submate:9000/bazarr/asr`
1. Enable language detection endpoint

### Jellyfin Integration

Webhook-based integration for automatic transcription:

1. Install Jellyfin Webhook plugin
1. Add webhook URL: `http://submate:9000/jellyfin/webhook`
1. Enable ItemAdded events

______________________________________________________________________

## Translation Backends

### Available Backends

| Backend | Variable | Free | Local |
|---------|----------|------|-------|
| Ollama | `SUBMATE__TRANSLATION__BACKEND=ollama` | Yes | Yes |
| OpenAI | `SUBMATE__TRANSLATION__BACKEND=openai` | No | No |
| Claude | `SUBMATE__TRANSLATION__BACKEND=claude` | No | No |
| Gemini | `SUBMATE__TRANSLATION__BACKEND=gemini` | No | No |

### Adding a New Backend

1. Add backend class in `submate/translation.py`:

```python
class NewBackend(TranslationBackendBase):
    def __init__(self, api_key: str, model: str):
        self.api_key = api_key
        self.model = model

    def translate(self, text: str, source_lang: str, target_lang: str) -> str:
        # Implementation
        return translated_text
```

2. Add settings to `TranslationSettings` in `config.py`
1. Add case to `TranslationService._init_backend()`

______________________________________________________________________

## Nix Packaging

### Package Structure

```
nix/packages/<name>/
├── package.nix          # Package definition
└── default.nix          # Wrapper (callPackage)
```

### Building Packages

```bash
# Build application
nix build .#submate

# Build Docker images
nix build .#docker-cpu
nix build .#docker-gpu

# Load Docker image
docker load < result
```

### Docker Image Usage

```bash
# Run server (default)
docker run -p 9000:9000 submate:cpu

# Run worker
docker run submate:cpu worker

# Run transcribe
docker run -v /media:/data submate:cpu transcribe /data/movie.mkv --sync

# With model persistence
docker run -v whisper-models:/root/.cache/huggingface submate:cpu
```

______________________________________________________________________

## Testing

### Test Structure

| File | Coverage |
|------|----------|
| `test_config.py` | Configuration validation |
| `test_cli.py` | CLI commands |
| `test_whisper.py` | Whisper model options |
| `test_jellyfin.py` | Jellyfin integration |
| `test_server.py` | FastAPI endpoints |
| `test_queue.py` | Task execution |

### Running Tests

```bash
# All tests
pytest tests/ -v

# With coverage
pytest tests/ --cov=submate

# Specific test file
pytest tests/test_config.py -v

# Specific test
pytest tests/test_config.py::test_config_from_env -v
```

### Test Conventions

- Use `monkeypatch.setenv()` for environment variables with `SUBMATE__` prefix
- Use `mocker.patch()` for mocking external services
- Use `tmp_path` fixture for temporary files

______________________________________________________________________

## Commit & Pull Request Guidelines

### Commit Message Format

```
{type}: {summary}

# Types: feat, fix, chore, refactor, test, docs, style

# Examples:
feat: add standalone subtitle translation CLI command
fix: respect SUBMATE__ env prefix in config
chore: clean up tests - remove over-mocked
refactor: consolidate tests from 16 to 10 files
```

### Before Pushing

1. Run `ruff format submate/ tests/`
1. Run `ruff check submate/ tests/`
1. Run `mypy submate/ --ignore-missing-imports`
1. Run `pytest tests/`
1. Run `nix flake check` for Nix changes

______________________________________________________________________

## Troubleshooting

### Common Issues

**"Module not found" for LLM backends**:

```bash
# Install optional dependencies
pip install submate[openai]  # or anthropic, ollama, gemini
```

**Environment variables not recognized**:

- Ensure `SUBMATE__` prefix is used
- Use double underscore `__` for nested settings

**Whisper model download fails**:

- Check internet connection
- Verify HuggingFace cache directory permissions

**GPU not detected**:

```bash
# Set device explicitly
export SUBMATE__WHISPER__DEVICE=cuda
```

**Bazarr connection refused**:

- Verify server is running: `submate server`
- Check port 9000 is accessible
- Verify endpoint URL in Bazarr settings
