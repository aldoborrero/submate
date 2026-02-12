# Config Loader Refactor - Pydantic Settings v2

**Goal:** Simplify config loading by leveraging pydantic-settings v2 built-in features for YAML and environment variable handling.

**Current Issues:**
- Custom `_EnvSettingsSource` class to work around JSON parsing
- Custom `YamlSettingsSource` class duplicating built-in functionality
- Dynamic `CustomConfig` class creation in `get_config()`
- Unnecessary complexity

**Solution:** Use pydantic-settings 2.10+ built-in `YamlConfigSettingsSource` and simplify the config loading flow.

---

## Configuration Precedence

From highest to lowest priority:
1. Environment variables (`SUBMATE__*`)
2. `.env` file (if exists)
3. YAML config file (`./config.yaml` or `--config` path)
4. Default values

## Auto-Detection Behavior

- If `--config` flag provided: use that path (error if not found)
- Otherwise: auto-load `./config.yaml` if it exists in current directory
- If neither: use defaults + environment variables only

---

## Implementation Changes

### 1. Simplify `Config` class

```python
from pydantic_settings import BaseSettings, SettingsConfigDict, YamlConfigSettingsSource

class Config(BaseSettings):
    model_config = SettingsConfigDict(
        env_prefix="SUBMATE__",
        env_nested_delimiter="__",
        env_file=".env",
        env_file_encoding="utf-8",
        case_sensitive=False,
        extra="ignore",
    )

    # Class variable for YAML path (set by get_config)
    _yaml_file: ClassVar[Path | None] = None

    @classmethod
    def settings_customise_sources(cls, settings_cls, init_settings, env_settings,
                                    dotenv_settings, file_secret_settings):
        sources = [init_settings, env_settings, dotenv_settings]

        if cls._yaml_file and cls._yaml_file.exists():
            sources.append(YamlConfigSettingsSource(settings_cls, yaml_file=cls._yaml_file))

        sources.append(file_secret_settings)
        return tuple(sources)
```

### 2. Simplify `get_config()`

```python
def get_config(config_file: Path | str | None = None) -> Config:
    """Load config from env vars, .env file, and optional YAML."""
    if isinstance(config_file, str):
        config_file = Path(config_file)

    # Auto-detect ./config.yaml if no explicit path
    if config_file is None and Path("config.yaml").exists():
        config_file = Path("config.yaml")

    # Validate explicit config file exists
    if config_file and not config_file.exists():
        raise FileNotFoundError(f"Config file not found: {config_file}")

    Config._yaml_file = config_file
    return Config()
```

### 3. Move YAML utilities to `config.py`

Keep these functions (move from `config_yaml.py`):
```python
def load_yaml_config(path: Path) -> dict[str, Any]:
    """Load configuration from YAML file."""
    if not path.exists():
        return {}
    with open(path, encoding="utf-8") as f:
        return yaml.safe_load(f) or {}

def save_yaml_config(path: Path, config: dict[str, Any]) -> None:
    """Save configuration to YAML file."""
    path.parent.mkdir(parents=True, exist_ok=True)
    with open(path, "w", encoding="utf-8") as f:
        yaml.dump(config, f, default_flow_style=False, sort_keys=False)
```

---

## Files Changed

| File | Action |
|------|--------|
| `submate/config.py` | Simplify, remove `_EnvSettingsSource`, add YAML utilities |
| `submate/config_yaml.py` | Delete entirely |
| `submate/server/handlers/settings/router.py` | Update import path |

## Code Removed

- `_EnvSettingsSource` class (~35 lines)
- `YamlSettingsSource` class (~80 lines)
- Dynamic `CustomConfig` class creation (~35 lines)
- `config_yaml.py` file

## Code Added

- `YamlConfigSettingsSource` usage (~5 lines)
- YAML utility functions moved (~15 lines)

**Net reduction:** ~130 lines of custom code replaced by built-in functionality.

---

## Validators Unchanged

The existing `@field_validator` decorators for pipe-separated lists remain unchanged:
- `parse_pipe_separated_folders`
- `parse_pipe_separated_libraries`
- `parse_pipe_separated_languages`
- `parse_json_kwargs`

These handle string-to-list conversion and work with the built-in env source.
