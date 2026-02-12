"""Pydantic models for Library API endpoints."""

from datetime import datetime

from pydantic import BaseModel, Field


class LibraryResponse(BaseModel):
    """Response model for a single library."""

    id: str = Field(description="Library ID")
    name: str = Field(description="Library display name")
    type: str = Field(description="Library type ('movies' or 'series')")
    target_languages: list[str] = Field(description="Target language codes for subtitle generation")
    skip_existing: bool = Field(description="Whether to skip items with existing subtitles")
    enabled: bool = Field(description="Whether the library is enabled for processing")
    last_synced: datetime | None = Field(default=None, description="Last sync timestamp")
    item_count: int = Field(default=0, description="Number of items in the library")


class LibraryListResponse(BaseModel):
    """Response model for listing libraries."""

    libraries: list[LibraryResponse] = Field(description="List of libraries")
    total: int = Field(description="Total number of libraries")


class LibraryUpdateRequest(BaseModel):
    """Request model for updating library settings."""

    target_languages: list[str] | None = Field(default=None, description="Target language codes")
    skip_existing: bool | None = Field(default=None, description="Whether to skip existing subtitles")
    enabled: bool | None = Field(default=None, description="Whether the library is enabled")
