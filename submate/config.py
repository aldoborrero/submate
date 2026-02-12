"""Configuration management using Pydantic Settings."""

import os
from pathlib import Path
from typing import Any, ClassVar

import yaml
from pydantic import BaseModel, Field, field_validator, model_validator
from pydantic_settings import BaseSettings, PydanticBaseSettingsSource, SettingsConfigDict, YamlConfigSettingsSource

from submate.types import Device, LanguageNamingType, TranslationBackend, WhisperImplementation, WhisperModel


def get_xdg_data_home() -> Path:
    """Get XDG_DATA_HOME directory.

    Returns:
        Path to XDG_DATA_HOME or ~/.local/share
    """
    xdg_data_home = os.getenv("XDG_DATA_HOME")
    if xdg_data_home:
        return Path(xdg_data_home)
    return Path.home() / ".local" / "share"


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


class WhisperSettings(BaseModel):
    """Whisper model and transcription settings."""

    model_config = SettingsConfigDict(str_strip_whitespace=True)

    # Core Whisper settings (works for faster-whisper, openai-whisper, etc.)
    model: str = Field(default=WhisperModel.MEDIUM, description="Whisper model size or path")
    device: Device = Field(default=Device.CPU, description="Device: cpu, cuda, auto")
    compute_type: str = Field(default="int8", description="Compute type for faster-whisper (int8, float16, etc.)")

    # Implementation-specific settings
    implementation: WhisperImplementation = Field(
        default=WhisperImplementation.FASTER_WHISPER,
        description="Whisper implementation: faster-whisper, openai-whisper, hf-whisper",
    )

    # Custom transcription arguments passed to Whisper
    transcribe_kwargs: dict[str, Any] = Field(
        default_factory=dict,
        description="Custom kwargs passed to transcribe (JSON string, e.g., beam_size, best_of)",
    )

    # Monitoring settings
    folders: list[str] = Field(default_factory=list, description="Folders to monitor")

    @field_validator("transcribe_kwargs", mode="before")
    @classmethod
    def parse_json_kwargs(cls, v: Any) -> dict[str, Any]:
        """Parse JSON string into dict for transcribe kwargs."""
        import json

        if isinstance(v, str) and v:
            try:
                parsed = json.loads(v)
                if isinstance(parsed, dict):
                    return parsed
                raise ValueError("transcribe_kwargs must be a JSON object")
            except json.JSONDecodeError as e:
                raise ValueError(f"Invalid JSON for transcribe_kwargs: {e}") from e
        if isinstance(v, dict):
            return v
        return {}

    @field_validator("folders", mode="before")
    @classmethod
    def parse_pipe_separated_folders(cls, v: Any) -> list[str]:
        """Parse pipe-separated string into list of folders."""
        if isinstance(v, str) and v:
            return [item.strip() for item in v.split("|") if item.strip()]
        if isinstance(v, list):
            return v
        return []

    @model_validator(mode="after")
    def validate_model_compatibility(self) -> "WhisperSettings":
        """Validate model based on implementation."""
        if self.implementation in {WhisperImplementation.FASTER_WHISPER, WhisperImplementation.OPENAI_WHISPER}:
            if self.model not in {model.value for model in WhisperModel}:
                raise ValueError(f"Invalid model '{self.model}' for {self.implementation.value}")
        elif self.implementation == WhisperImplementation.HF_WHISPER:
            if not self.model or "/" not in self.model:
                raise ValueError(f"Invalid HF model format: '{self.model}'")
        return self


class StableTsSettings(BaseModel):
    """Stable-ts subtitle generation settings."""

    word_level_highlight: bool = Field(default=False, description="Enable word-level highlighting in VTT")
    custom_regroup: str | bool = Field(
        default="cm_sl=84_sl=42++++++1", description="Regrouping pattern or False to disable"
    )
    suppress_silence: bool = Field(default=True, description="Suppress silence in timestamps")
    min_word_duration: float = Field(default=0.1, description="Minimum word duration in seconds")

    @field_validator("custom_regroup", mode="before")
    @classmethod
    def parse_regroup(cls, v: Any) -> str | bool:
        """Parse custom_regroup field, handling 'false' strings."""
        if isinstance(v, str):
            if v.lower() in ("false", "off", "0", "no", ""):
                return False
            return v
        if isinstance(v, bool):
            return v
        # Default for other types
        return str(v) if v is not None else False


class ServerSettings(BaseModel):
    """Server and processing settings."""

    address: str = Field(default="0.0.0.0", description="Server address to bind to")
    port: int = Field(default=9000, description="Server port")

    concurrent_transcriptions: int = Field(default=2, description="Number of concurrent transcriptions")
    process_on_add: bool = Field(default=True, description="Process media when added to library")
    process_on_play: bool = Field(default=False, description="Process media when playback starts")

    # Feature enable/disable flags
    bazarr_enabled: bool = Field(default=True, description="Enable Bazarr ASR integration")
    jellyfin_enabled: bool = Field(default=True, description="Enable Jellyfin webhook integration")
    status_enabled: bool = Field(default=True, description="Enable status/queue endpoints")

    # Bazarr settings
    bazarr_keep_model_loaded: bool = Field(
        default=True,
        description="Keep model loaded for faster Bazarr responses",
    )
    bazarr_model_idle_timeout: int = Field(
        default=300,
        description="Seconds before unloading idle model",
    )


class PathMappingSettings(BaseModel):
    """Path mapping settings for Docker deployments."""

    enabled: bool = Field(default=False, description="Enable path mapping for Docker")
    from_path: str = Field(default="", description="Source path for mapping")
    to_path: str = Field(default="", description="Destination path for mapping")


class JellyfinSettings(BaseModel):
    """Jellyfin media server integration settings."""

    model_config = SettingsConfigDict(str_strip_whitespace=True)

    server_url: str = Field(default="", description="Jellyfin server URL")
    api_key: str = Field(default="", description="Jellyfin API key")
    libraries: list[str] = Field(default_factory=list, description="Jellyfin libraries to refresh")

    @field_validator("libraries", mode="before")
    @classmethod
    def parse_pipe_separated_libraries(cls, v: Any) -> list[str]:
        """Parse pipe-separated string into list of libraries."""
        if isinstance(v, str) and v:
            return [item.strip() for item in v.split("|") if item.strip()]
        if isinstance(v, list):
            return v
        return []


class QueueSettings(BaseModel):
    """Queue and retry settings."""

    db_path: str = Field(default="", description="Path to queue database")
    max_retries: int = Field(default=3, description="Maximum retry attempts")
    retry_delay: int = Field(default=5, description="Retry delay in seconds")


class SubtitleSettings(BaseModel):
    """Subtitle generation and language settings with comprehensive skip logic."""

    # Existing settings
    force_detected_language_to: str = Field(default="", description="Force detected language to this code")
    append_credits: bool = Field(default=False, description="Append credits to subtitles")

    # Skip conditions - Group 1: Target subtitle checks
    skip_if_target_subtitle_exists: bool = Field(
        default=True,
        description="Skip if target language subtitle already exists (internal OR external)",
    )
    skip_if_external_subtitles_exist: bool = Field(
        default=False,
        description="Skip if any external subtitle file exists",
    )
    skip_if_internal_subtitle_language: str = Field(
        default="",
        description="Skip if internal subtitle exists in this language (e.g., 'eng')",
    )

    # Skip conditions - Group 2: Language-based skipping
    skip_subtitle_languages: list[str] = Field(
        default_factory=list,
        description="Skip if subtitle in any of these languages exists (pipe-separated)",
    )
    skip_if_audio_languages: list[str] = Field(
        default_factory=list,
        description="Skip if audio track is in any of these languages (pipe-separated)",
    )
    skip_unknown_language: bool = Field(
        default=False,
        description="Skip if language cannot be determined",
    )

    # Skip conditions - Group 3: Preference-based skipping
    preferred_audio_languages: list[str] = Field(
        default_factory=list,
        description="Preferred audio languages in order (pipe-separated)",
    )
    limit_to_preferred_audio_languages: bool = Field(
        default=False,
        description="Skip if no preferred audio language found",
    )

    # Skip conditions - Group 4: Audio file specific
    lrc_for_audio_files: bool = Field(
        default=True,
        description="Generate LRC for audio files instead of SRT",
    )

    # Skip conditions - Group 5: Subgen-specific
    only_skip_if_subgen_subtitle: bool = Field(
        default=False,
        description="Only skip if subtitle was generated by subgen (has .subgen in name)",
    )
    skip_if_no_language_but_subtitles_exist: bool = Field(
        default=False,
        description="Skip if language unknown but any subtitles exist",
    )

    # Subtitle naming options
    language_naming_type: LanguageNamingType = Field(
        default=LanguageNamingType.ISO_639_2_B,
        description="Language code format: iso_639_1, iso_639_2_t, iso_639_2_b, name, native",
    )
    include_subgen_marker: bool = Field(
        default=False,
        description="Include .subgen in subtitle filename (e.g., movie.subgen.eng.srt)",
    )
    include_model_in_filename: bool = Field(
        default=False,
        description="Include model name in subtitle filename (e.g., movie.medium.eng.srt)",
    )

    @field_validator(
        "skip_subtitle_languages",
        "skip_if_audio_languages",
        "preferred_audio_languages",
        mode="before",
    )
    @classmethod
    def parse_pipe_separated_languages(cls, v: Any) -> list[str]:
        """Parse pipe-separated string into list of language codes."""
        if isinstance(v, str) and v:
            return [item.strip() for item in v.split("|") if item.strip()]
        if isinstance(v, list):
            return v
        return []


class TranslationSettings(BaseModel):
    """Translation settings for multi-language subtitle translation via LLM APIs."""

    backend: TranslationBackend = Field(
        default=TranslationBackend.OLLAMA,
        description="Translation backend: ollama (local/free), claude, openai, gemini",
    )

    # Ollama settings (default - free, local, private)
    ollama_model: str = Field(default="llama3.2", description="Ollama model for translation")
    ollama_url: str = Field(default="http://localhost:11434", description="Ollama API URL")

    # Claude/Anthropic settings
    anthropic_api_key: str = Field(default="", description="Anthropic API key for Claude")
    claude_model: str = Field(default="claude-sonnet-4-20250514", description="Claude model for translation")

    # OpenAI settings
    openai_api_key: str = Field(default="", description="OpenAI API key")
    openai_model: str = Field(default="gpt-4o-mini", description="OpenAI model for translation")

    # Google Gemini settings
    gemini_api_key: str = Field(default="", description="Google Gemini API key")
    gemini_model: str = Field(default="gemini-2.0-flash", description="Gemini model for translation")

    # Chunking settings
    chunk_size: int = Field(
        default=50,
        description="Number of subtitle blocks per translation batch (for context window limits)",
    )

    def validate_for_target(self, target_lang: str | None) -> None:
        """Validate backend configuration only if LLM translation is needed.

        English translations use Whisper's built-in translate - no LLM required.
        Non-English translations require a properly configured LLM backend.

        Args:
            target_lang: Target language code (e.g., 'es', 'fr', 'en')

        Raises:
            ValueError: If LLM backend is needed but not properly configured
        """
        from submate.language import LanguageCode

        # No validation needed if no translation or translating to English
        if not target_lang or LanguageCode.from_string(target_lang) == LanguageCode.ENGLISH:
            return

        # LLM translation needed - validate backend
        match self.backend:
            case TranslationBackend.OLLAMA:
                pass  # Ollama has no API key, will fail at runtime if not running
            case TranslationBackend.CLAUDE:
                if not self.anthropic_api_key:
                    raise ValueError(
                        f"Translation to '{target_lang}' requires LLM. "
                        f"Set SUBMATE__TRANSLATION__ANTHROPIC_API_KEY or use SUBMATE__TRANSLATION__BACKEND=ollama"
                    )
            case TranslationBackend.OPENAI:
                if not self.openai_api_key:
                    raise ValueError(
                        f"Translation to '{target_lang}' requires LLM. "
                        f"Set SUBMATE__TRANSLATION__OPENAI_API_KEY or use SUBMATE__TRANSLATION__BACKEND=ollama"
                    )
            case TranslationBackend.GEMINI:
                if not self.gemini_api_key:
                    raise ValueError(
                        f"Translation to '{target_lang}' requires LLM. "
                        f"Set SUBMATE__TRANSLATION__GEMINI_API_KEY or use SUBMATE__TRANSLATION__BACKEND=ollama"
                    )


class Config(BaseSettings):
    """Application configuration with Pydantic validation.

    Configuration is loaded from (in order of precedence):
    1. Environment variables
    2. .env file (if present)
    3. YAML configuration file (if provided or ./config.yaml exists)
    4. Default values

    Nested configuration uses __ delimiter (e.g., WHISPER__MODEL).
    Pipe-separated lists are parsed for folders and libraries fields.
    """

    model_config = SettingsConfigDict(
        env_file=".env",
        env_file_encoding="utf-8",
        env_prefix="SUBMATE__",
        case_sensitive=False,
        extra="ignore",  # Ignore unknown env vars
        env_nested_delimiter="__",
        enable_decoding=False,  # Let validators handle string parsing (pipe-separated lists, etc.)
    )

    # Class variable for YAML path (set by get_config before instantiation)
    _yaml_file: ClassVar[Path | None] = None

    whisper: WhisperSettings = Field(default_factory=WhisperSettings)
    stable_ts: StableTsSettings = Field(default_factory=StableTsSettings)
    server: ServerSettings = Field(default_factory=ServerSettings)
    path_mapping: PathMappingSettings = Field(default_factory=PathMappingSettings)
    jellyfin: JellyfinSettings = Field(default_factory=JellyfinSettings)
    queue: QueueSettings = Field(default_factory=QueueSettings)
    subtitle: SubtitleSettings = Field(default_factory=SubtitleSettings)
    translation: TranslationSettings = Field(default_factory=TranslationSettings)

    # Feature flags (kept at top level)
    debug: bool = Field(default=False, description="Enable debug logging")
    clear_vram_on_complete: bool = Field(default=False, description="Clear VRAM after transcription")

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

    @field_validator("queue", mode="before")
    @classmethod
    def set_default_queue_path(cls, v: Any) -> dict[str, Any] | QueueSettings:
        """Set default queue database path using XDG."""
        if isinstance(v, QueueSettings):
            if not v.db_path or v.db_path == "":
                data_dir = get_xdg_data_home() / "subgen"
                data_dir.mkdir(parents=True, exist_ok=True)
                v.db_path = str(data_dir / "queue.db")
            return v

        # Handle dict input
        if isinstance(v, dict):
            if not v.get("db_path"):
                data_dir = get_xdg_data_home() / "subgen"
                data_dir.mkdir(parents=True, exist_ok=True)
                v["db_path"] = str(data_dir / "queue.db")
            return v

        # Handle None or empty - create default
        data_dir = get_xdg_data_home() / "subgen"
        data_dir.mkdir(parents=True, exist_ok=True)
        return {"db_path": str(data_dir / "queue.db")}


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
