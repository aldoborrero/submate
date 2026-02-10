"""Pydantic models for Bazarr integration."""

from pydantic import BaseModel, Field


class LanguageDetectionResponse(BaseModel):
    """Response for language detection endpoint."""

    detected_language: str = Field(description="Human-readable language name")
    language_code: str = Field(description="ISO 639-1 language code")
