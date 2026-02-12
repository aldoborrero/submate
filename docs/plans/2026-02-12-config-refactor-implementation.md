# Config Refactor Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Simplify config loading by using pydantic-settings v2 built-in YAML support, removing ~130 lines of custom code.

**Architecture:** Replace custom `_EnvSettingsSource` and `YamlSettingsSource` with built-in `YamlConfigSettingsSource`. Move YAML utilities to `config.py` and delete `config_yaml.py`.

**Tech Stack:** Python 3.13, pydantic-settings 2.10+, PyYAML

---

## Task 1: Update config.py - Remove Custom Sources, Add YAML Utilities

**Files:**
- Modify: `submate/config.py`

**Step 1: Add yaml import and YAML utility functions**

At the top of `config.py`, add after existing imports:

```python
import yaml
```

After the `get_xdg_data_home()` function (around line 62), add:

```python
def load_yaml_config(path: Path) -> dict[str, Any]:
    """Load configuration from YAML file.

    Args:
        path: Path to YAML configuration file

    Returns:
        Configuration dictionary, empty if file doesn't exist
    """
    if not path.exists():
        return {}

    with open(path, encoding="utf-8") as f:
        return yaml.safe_load(f) or {}


def save_yaml_config(path: Path, config: dict[str, Any]) -> None:
    """Save configuration to YAML file.

    Args:
        path: Path to YAML configuration file
        config: Configuration dictionary to save
    """
    path.parent.mkdir(parents=True, exist_ok=True)
    with open(path, "w", encoding="utf-8") as f:
        yaml.dump(config, f, default_flow_style=False, sort_keys=False)
```

**Step 2: Remove the `_EnvSettingsSource` class**

Delete the entire `_EnvSettingsSource` class (lines 13-49).

**Step 3: Update imports**

Add to imports:

```python
from typing import Any, ClassVar

from pydantic_settings import BaseSettings, PydanticBaseSettingsSource, SettingsConfigDict, YamlConfigSettingsSource
```

Remove `os` from imports if no longer needed (check if only used by deleted code).

**Step 4: Add ClassVar for yaml_file to Config class**

In the `Config` class, add before the fields:

```python
    # Class variable for YAML path (set by get_config before instantiation)
    _yaml_file: ClassVar[Path | None] = None
```

**Step 5: Replace settings_customise_sources in Config class**

Replace the existing `settings_customise_sources` method with:

```python
    @classmethod
    def settings_customise_sources(
        cls,
        settings_cls: type[BaseSettings],
        init_settings: PydanticBaseSettingsSource,
        env_settings: PydanticBaseSettingsSource,
        dotenv_settings: PydanticBaseSettingsSource,
        file_secret_settings: PydanticBaseSettingsSource,
    ) -> tuple[PydanticBaseSettingsSource, ...]:
        """Configure settings sources with optional YAML support."""
        sources: list[PydanticBaseSettingsSource] = [
            init_settings,
            env_settings,
            dotenv_settings,
        ]

        # Add YAML source if path is set and file exists
        if cls._yaml_file and cls._yaml_file.exists():
            sources.append(YamlConfigSettingsSource(settings_cls, yaml_file=cls._yaml_file))

        sources.append(file_secret_settings)
        return tuple(sources)
```

**Step 6: Replace get_config function**

Replace the entire `get_config` function with:

```python
def get_config(config_file: Path | str | None = None) -> Config:
    """Load configuration from environment, .env file, and optional YAML.

    Configuration sources are applied in order of precedence (highest first):
    1. Environment variables (always override)
    2. .env file (if exists)
    3. YAML configuration file (if provided or ./config.yaml exists)
    4. Default values

    Args:
        config_file: Optional path to YAML configuration file.
                    If not provided, auto-detects ./config.yaml in current directory.

    Returns:
        Populated Config instance with validation

    Raises:
        FileNotFoundError: If explicit config_file path doesn't exist
    """
    if isinstance(config_file, str):
        config_file = Path(config_file)

    # Auto-detect ./config.yaml if no explicit path provided
    if config_file is None and Path("config.yaml").exists():
        config_file = Path("config.yaml")

    # Validate explicit config file exists
    if config_file is not None and not config_file.exists():
        raise FileNotFoundError(f"Config file not found: {config_file}")

    # Set class variable before instantiation
    Config._yaml_file = config_file

    return Config()
```

**Step 7: Remove old yaml_path import**

Remove the import of `YamlSettingsSource` from `submate.config_yaml` (if present).

**Step 8: Run tests to verify**

Run: `pytest tests/test_config.py -v`
Expected: All tests pass

**Step 9: Commit**

```bash
git add submate/config.py
git commit -m "refactor(config): use built-in YamlConfigSettingsSource

- Remove custom _EnvSettingsSource class
- Add load_yaml_config and save_yaml_config utilities
- Simplify get_config with auto-detection of ./config.yaml
- Use pydantic-settings built-in YAML support"
```

---

## Task 2: Update settings router import

**Files:**
- Modify: `submate/server/handlers/settings/router.py`

**Step 1: Update import**

Change line 10 from:
```python
from submate.config_yaml import load_yaml_config, save_yaml_config
```

To:
```python
from submate.config import load_yaml_config, save_yaml_config
```

**Step 2: Run tests**

Run: `pytest tests/test_settings_api.py -v`
Expected: All tests pass

**Step 3: Commit**

```bash
git add submate/server/handlers/settings/router.py
git commit -m "refactor(settings): update yaml config import path"
```

---

## Task 3: Update YAML config tests

**Files:**
- Modify: `tests/test_config_yaml.py`

**Step 1: Update imports in test file**

Change:
```python
from submate.config_yaml import load_yaml_config, save_yaml_config
```

To:
```python
from submate.config import load_yaml_config, save_yaml_config
```

**Step 2: Remove YamlSettingsSource test**

Delete the test `test_yaml_settings_source_get_field_value` (lines 218-253) as `YamlSettingsSource` class no longer exists.

**Step 3: Update tests that use yaml_path parameter**

The `get_config` function signature changed from `get_config(config_file, yaml_path)` to `get_config(config_file)`.

Update these tests to use `config_file` parameter instead of `yaml_path`:

- `test_yaml_settings_source_loads_into_config`: Change `get_config(yaml_path=yaml_path)` to `get_config(config_file=yaml_path)`
- `test_env_vars_override_yaml_values`: Change `get_config(yaml_path=yaml_path)` to `get_config(config_file=yaml_path)`
- `test_yaml_partial_config_uses_defaults`: Change `get_config(yaml_path=yaml_path)` to `get_config(config_file=yaml_path)`
- `test_yaml_with_nested_settings`: Change `get_config(yaml_path=yaml_path)` to `get_config(config_file=yaml_path)`
- `test_yaml_with_list_fields`: Change `get_config(yaml_path=yaml_path)` to `get_config(config_file=yaml_path)`
- `test_yaml_settings_source_with_nonexistent_file`: Change `get_config(yaml_path=...)` to `get_config(config_file=...)` and expect `FileNotFoundError`
- `test_yaml_string_path_accepted`: Change `get_config(yaml_path=yaml_path_str)` to `get_config(config_file=yaml_path_str)`
- `test_combined_env_file_and_yaml`: Remove `config_file` parameter (we no longer support separate .env path), just use `get_config(config_file=yaml_file)`

**Step 4: Update nonexistent file test**

The test `test_yaml_settings_source_with_nonexistent_file` should now expect `FileNotFoundError`:

```python
def test_yaml_settings_source_with_nonexistent_file():
    """Test that nonexistent YAML file raises FileNotFoundError."""
    from submate.config import get_config

    with pytest.raises(FileNotFoundError):
        get_config(config_file=Path("/nonexistent/config.yaml"))
```

**Step 5: Simplify combined test**

Update `test_combined_env_file_and_yaml`:

```python
def test_combined_env_file_and_yaml(monkeypatch, tmp_path):
    """Test using env vars and YAML file together."""
    from submate.config import get_config

    # Set env var
    monkeypatch.setenv("SUBMATE__SERVER__PORT", "7777")

    # Create YAML file
    yaml_content = """
whisper:
  model: "tiny"
debug: true
"""
    yaml_file = tmp_path / "config.yaml"
    yaml_file.write_text(yaml_content)

    config = get_config(config_file=yaml_file)

    # Env var should override
    assert config.server.port == 7777
    # YAML values
    assert config.whisper.model == "tiny"
    assert config.debug is True
```

**Step 6: Run tests**

Run: `pytest tests/test_config_yaml.py -v`
Expected: All tests pass

**Step 7: Commit**

```bash
git add tests/test_config_yaml.py
git commit -m "test(config): update tests for simplified config loading"
```

---

## Task 4: Delete config_yaml.py

**Files:**
- Delete: `submate/config_yaml.py`

**Step 1: Delete the file**

```bash
rm submate/config_yaml.py
```

**Step 2: Run all config tests**

Run: `pytest tests/test_config.py tests/test_config_yaml.py -v`
Expected: All tests pass

**Step 3: Run full test suite**

Run: `pytest tests/ -v`
Expected: All tests pass

**Step 4: Commit**

```bash
git add -A
git commit -m "refactor(config): delete config_yaml.py

YAML utilities moved to config.py. Custom YamlSettingsSource replaced
by built-in YamlConfigSettingsSource from pydantic-settings."
```

---

## Task 5: Verify and cleanup

**Step 1: Run type checker**

Run: `mypy submate/config.py --ignore-missing-imports`
Expected: No errors

**Step 2: Run linter**

Run: `ruff check submate/config.py`
Expected: No errors

**Step 3: Format code**

Run: `ruff format submate/config.py tests/test_config_yaml.py`

**Step 4: Run full test suite one more time**

Run: `pytest tests/ -v`
Expected: All tests pass

**Step 5: Final commit if any formatting changes**

```bash
git add -A
git commit -m "style: format config files" || echo "No changes to commit"
```

---

## Summary

| Before | After |
|--------|-------|
| `_EnvSettingsSource` class (35 lines) | Removed - use built-in |
| `YamlSettingsSource` class (80 lines) | Removed - use built-in `YamlConfigSettingsSource` |
| Dynamic `CustomConfig` class (35 lines) | Removed - use `_yaml_file` class var |
| `config_yaml.py` file | Deleted |
| YAML utilities in separate file | Moved to `config.py` |

**Net reduction:** ~130 lines of custom code
