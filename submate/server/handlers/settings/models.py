"""Pydantic models for Settings API endpoints."""

from pydantic import BaseModel, Field


class JellyfinSettings(BaseModel):
    """Jellyfin connection settings."""

    server_url: str = Field(default="", description="Jellyfin server URL")
    api_key: str = Field(default="", description="Jellyfin API key")


class WhisperSettings(BaseModel):
    """Whisper transcription settings."""

    model: str = Field(default="medium", description="Whisper model size")
    device: str = Field(default="cpu", description="Device to use (cpu, cuda, auto)")
    compute_type: str = Field(default="int8", description="Compute type for faster-whisper")


class TranslationSettings(BaseModel):
    """LLM translation settings."""

    backend: str = Field(default="ollama", description="Translation backend")
    ollama_url: str = Field(default="http://localhost:11434", description="Ollama API URL")
    ollama_model: str = Field(default="llama3.2", description="Ollama model")
    openai_api_key: str = Field(default="", description="OpenAI API key")
    openai_model: str = Field(default="gpt-4o-mini", description="OpenAI model")
    anthropic_api_key: str = Field(default="", description="Anthropic API key")
    claude_model: str = Field(default="claude-sonnet-4-20250514", description="Claude model")
    gemini_api_key: str = Field(default="", description="Gemini API key")
    gemini_model: str = Field(default="gemini-2.0-flash", description="Gemini model")


class NotificationSettings(BaseModel):
    """Notification settings."""

    webhook_url: str | None = Field(default=None, description="Generic webhook URL")
    ntfy_url: str | None = Field(default=None, description="ntfy server URL")
    ntfy_topic: str | None = Field(default=None, description="ntfy topic")
    apprise_urls: list[str] = Field(default_factory=list, description="Apprise notification URLs")


class SettingsResponse(BaseModel):
    """Response model for settings endpoints."""

    jellyfin: JellyfinSettings = Field(default_factory=JellyfinSettings)
    whisper: WhisperSettings = Field(default_factory=WhisperSettings)
    translation: TranslationSettings = Field(default_factory=TranslationSettings)
    notifications: NotificationSettings = Field(default_factory=NotificationSettings)


class SettingsUpdateRequest(BaseModel):
    """Request model for updating settings."""

    jellyfin: JellyfinSettings | None = Field(default=None, description="Jellyfin settings to update")
    whisper: WhisperSettings | None = Field(default=None, description="Whisper settings to update")
    translation: TranslationSettings | None = Field(default=None, description="Translation settings to update")
    notifications: NotificationSettings | None = Field(default=None, description="Notification settings to update")


class TestConnectionResponse(BaseModel):
    """Response model for connection test endpoints."""

    success: bool = Field(description="Whether the test was successful")
    message: str = Field(description="Human-readable result message")
    details: dict = Field(default_factory=dict, description="Additional details")
