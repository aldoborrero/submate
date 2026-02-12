"""Pydantic models for Subtitles API endpoints."""

from datetime import datetime

from pydantic import BaseModel, Field


class SubtitleResponse(BaseModel):
    """Response model for a single subtitle."""

    id: int = Field(description="Subtitle ID")
    item_id: str = Field(description="Parent item ID")
    language: str = Field(description="Language code")
    source: str = Field(description="Source type ('external' or 'generated')")
    path: str = Field(description="File path to the subtitle")
    created_at: datetime = Field(description="Creation timestamp")


class SubtitleListResponse(BaseModel):
    """Response model for listing subtitles."""

    subtitles: list[SubtitleResponse] = Field(description="List of subtitles")
    total: int = Field(description="Total number of subtitles")


class SubtitleContentResponse(BaseModel):
    """Response model for subtitle content."""

    language: str = Field(description="Language code")
    content: str = Field(description="Subtitle file content")
    format: str = Field(description="Subtitle format ('srt', 'ass', 'vtt')")


class SubtitleUpdateRequest(BaseModel):
    """Request model for updating subtitle content."""

    content: str = Field(description="New subtitle content")


class SyncResponse(BaseModel):
    """Response model for subtitle sync operation."""

    success: bool = Field(description="Whether the sync was successful")
    message: str = Field(description="Status message")
