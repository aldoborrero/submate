"""Pydantic models for Items API endpoints."""

from datetime import datetime

from pydantic import BaseModel, Field


class ItemResponse(BaseModel):
    """Response model for a single media item."""

    id: str = Field(description="Item ID")
    library_id: str = Field(description="Parent library ID")
    type: str = Field(description="Item type ('movie', 'series', or 'episode')")
    title: str = Field(description="Item title")
    path: str = Field(description="File path to the media")
    series_id: str | None = Field(default=None, description="Series ID for episodes")
    series_name: str | None = Field(default=None, description="Series name for episodes")
    season_num: int | None = Field(default=None, description="Season number for episodes")
    episode_num: int | None = Field(default=None, description="Episode number for episodes")
    poster_url: str | None = Field(default=None, description="URL to poster image")
    last_synced: datetime = Field(description="Last sync timestamp")
    subtitle_languages: list[str] = Field(default_factory=list, description="Languages with existing subtitles")


class ItemListResponse(BaseModel):
    """Response model for listing items with pagination."""

    items: list[ItemResponse] = Field(description="List of items")
    total: int = Field(description="Total number of items")
    page: int = Field(description="Current page number")
    page_size: int = Field(description="Number of items per page")


class SeriesDetailResponse(ItemResponse):
    """Response model for series detail with episodes."""

    episodes: list[ItemResponse] = Field(default_factory=list, description="Episodes in the series")
    season_count: int = Field(default=0, description="Number of seasons")
    episode_count: int = Field(default=0, description="Total number of episodes")
