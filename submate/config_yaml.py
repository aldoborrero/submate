"""YAML configuration file utilities with Pydantic integration."""

from pathlib import Path
from typing import Any

import yaml
from pydantic.fields import FieldInfo
from pydantic_settings import BaseSettings, PydanticBaseSettingsSource


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


class YamlSettingsSource(PydanticBaseSettingsSource):
    """Pydantic settings source that loads configuration from a YAML file.

    This source allows YAML files to be used as a configuration source
    for Pydantic Settings models. It integrates with the existing
    settings_customise_sources mechanism.

    Precedence (highest to lowest):
    1. Environment variables (override everything)
    2. YAML configuration file
    3. Default values

    Example usage:
        class Config(BaseSettings):
            @classmethod
            def settings_customise_sources(cls, settings_cls, ...):
                yaml_source = YamlSettingsSource(settings_cls, yaml_path)
                return (init_settings, env_settings, yaml_source, ...)
    """

    def __init__(
        self,
        settings_cls: type[BaseSettings],
        yaml_path: Path | None = None,
    ) -> None:
        """Initialize the YAML settings source.

        Args:
            settings_cls: The Pydantic Settings class
            yaml_path: Path to the YAML configuration file (optional)
        """
        super().__init__(settings_cls)
        self.yaml_path = yaml_path
        self._yaml_data: dict[str, Any] | None = None

    def _load_yaml(self) -> dict[str, Any]:
        """Load and cache YAML data from file.

        Returns:
            Configuration dictionary from YAML file, or empty dict if not available
        """
        if self._yaml_data is None:
            if self.yaml_path and self.yaml_path.exists():
                self._yaml_data = load_yaml_config(self.yaml_path)
            else:
                self._yaml_data = {}
        return self._yaml_data

    def get_field_value(
        self,
        field: FieldInfo,
        field_name: str,
    ) -> tuple[Any, str, bool]:
        """Get value for a specific field from YAML data.

        Args:
            field: The Pydantic field info
            field_name: Name of the field

        Returns:
            Tuple of (value, field_name, is_complex)
            - value: The value from YAML or None if not found
            - field_name: The field name
            - is_complex: Whether the value needs deep validation (True for nested dicts)
        """
        yaml_data = self._load_yaml()
        if field_name in yaml_data:
            value = yaml_data[field_name]
            # Mark nested dicts as complex so Pydantic validates them properly
            is_complex = isinstance(value, dict)
            return value, field_name, is_complex
        return None, field_name, False

    def __call__(self) -> dict[str, Any]:
        """Return all YAML configuration data.

        This is called by Pydantic to get all settings from this source.

        Returns:
            The complete YAML configuration dictionary
        """
        return self._load_yaml()
