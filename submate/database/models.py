"""SQLAlchemy models for Submate UI database."""

import json
from datetime import datetime
from typing import Any

from sqlalchemy import Boolean, DateTime, ForeignKey, Index, Integer, String, Text
from sqlalchemy.orm import DeclarativeBase, Mapped, mapped_column, relationship


class Base(DeclarativeBase):
    """Base class for all models."""

    pass


class Library(Base):
    """Jellyfin library metadata."""

    __tablename__ = "libraries"

    id: Mapped[str] = mapped_column(String(64), primary_key=True)
    name: Mapped[str] = mapped_column(String(255), nullable=False)
    type: Mapped[str] = mapped_column(String(20), nullable=False)  # 'movies' or 'series'
    _target_languages: Mapped[str] = mapped_column("target_languages", Text, nullable=False, default="[]")
    skip_existing: Mapped[bool] = mapped_column(Boolean, nullable=False, default=True)
    enabled: Mapped[bool] = mapped_column(Boolean, nullable=False, default=True)
    last_synced: Mapped[datetime | None] = mapped_column(DateTime, nullable=True)

    # Relationships
    items: Mapped[list["Item"]] = relationship("Item", back_populates="library", cascade="all, delete-orphan")

    def __init__(self, **kwargs: Any) -> None:
        # Handle target_languages as list
        if "target_languages" in kwargs and isinstance(kwargs["target_languages"], list):
            kwargs["_target_languages"] = json.dumps(kwargs["target_languages"])
            del kwargs["target_languages"]
        super().__init__(**kwargs)

    @property
    def target_languages(self) -> list[str]:
        """Get target languages as list."""
        if isinstance(self._target_languages, str):
            result: list[str] = json.loads(self._target_languages)
            return result
        return self._target_languages

    @target_languages.setter
    def target_languages(self, value: list[str]) -> None:
        """Set target languages from list."""
        self._target_languages = json.dumps(value)


class Item(Base):
    """Media item (movie or episode)."""

    __tablename__ = "items"

    id: Mapped[str] = mapped_column(String(64), primary_key=True)
    library_id: Mapped[str] = mapped_column(String(64), ForeignKey("libraries.id", ondelete="CASCADE"), nullable=False)
    type: Mapped[str] = mapped_column(String(20), nullable=False)  # 'movie' or 'episode'
    title: Mapped[str] = mapped_column(String(500), nullable=False)
    path: Mapped[str] = mapped_column(String(1000), nullable=False, unique=True)
    series_id: Mapped[str | None] = mapped_column(String(64), nullable=True)
    series_name: Mapped[str | None] = mapped_column(String(500), nullable=True)
    season_num: Mapped[int | None] = mapped_column(Integer, nullable=True)
    episode_num: Mapped[int | None] = mapped_column(Integer, nullable=True)
    poster_url: Mapped[str | None] = mapped_column(String(1000), nullable=True)
    last_synced: Mapped[datetime] = mapped_column(DateTime, nullable=False, default=datetime.utcnow)

    # Relationships
    library: Mapped["Library"] = relationship("Library", back_populates="items")
    subtitles: Mapped[list["Subtitle"]] = relationship("Subtitle", back_populates="item", cascade="all, delete-orphan")
    jobs: Mapped[list["Job"]] = relationship("Job", back_populates="item", cascade="all, delete-orphan")

    __table_args__ = (
        Index("idx_items_library", "library_id"),
        Index("idx_items_series", "series_id"),
    )


class Subtitle(Base):
    """Subtitle file metadata."""

    __tablename__ = "subtitles"

    id: Mapped[int] = mapped_column(Integer, primary_key=True, autoincrement=True)
    item_id: Mapped[str] = mapped_column(String(64), ForeignKey("items.id", ondelete="CASCADE"), nullable=False)
    language: Mapped[str] = mapped_column(String(10), nullable=False)
    source: Mapped[str] = mapped_column(String(20), nullable=False)  # 'external' or 'generated'
    path: Mapped[str] = mapped_column(String(1000), nullable=False)
    created_at: Mapped[datetime] = mapped_column(DateTime, nullable=False, default=datetime.utcnow)

    # Relationships
    item: Mapped["Item"] = relationship("Item", back_populates="subtitles")

    __table_args__ = (Index("idx_subtitles_item", "item_id"),)


class Job(Base):
    """Transcription job record."""

    __tablename__ = "jobs"

    id: Mapped[str] = mapped_column(String(64), primary_key=True)
    item_id: Mapped[str] = mapped_column(String(64), ForeignKey("items.id", ondelete="CASCADE"), nullable=False)
    language: Mapped[str] = mapped_column(String(10), nullable=False)
    status: Mapped[str] = mapped_column(String(20), nullable=False)  # 'pending', 'running', 'completed', 'failed'
    error: Mapped[str | None] = mapped_column(Text, nullable=True)
    created_at: Mapped[datetime] = mapped_column(DateTime, nullable=False, default=datetime.utcnow)
    started_at: Mapped[datetime | None] = mapped_column(DateTime, nullable=True)
    completed_at: Mapped[datetime | None] = mapped_column(DateTime, nullable=True)

    # Relationships
    item: Mapped["Item"] = relationship("Item", back_populates="jobs")

    __table_args__ = (
        Index("idx_jobs_item", "item_id"),
        Index("idx_jobs_status", "status"),
    )
