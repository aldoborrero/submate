"""YAML configuration file utilities."""

from pathlib import Path
from typing import Any

import yaml


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
