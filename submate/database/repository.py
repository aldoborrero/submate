"""Database repository layer for Submate UI.

Provides repository classes for CRUD operations on database models.
"""

from datetime import UTC, datetime

from sqlalchemy import func
from sqlalchemy.orm import Session

from submate.database.models import Item, Job, Library, Subtitle


class LibraryRepository:
    """Repository for Library CRUD operations."""

    def __init__(self, session: Session) -> None:
        """Initialize repository with a database session.

        Args:
            session: SQLAlchemy session instance.
        """
        self.session = session

    def create(
        self,
        id: str,
        name: str,
        type: str,
        target_languages: list[str],
        skip_existing: bool = True,
        enabled: bool = True,
    ) -> Library:
        """Create a new library.

        Args:
            id: Jellyfin library ID.
            name: Library display name.
            type: Library type ('movies' or 'series').
            target_languages: List of target language codes.
            skip_existing: Whether to skip existing subtitles.
            enabled: Whether the library is enabled for processing.

        Returns:
            The created Library instance.
        """
        library = Library(
            id=id,
            name=name,
            type=type,
            target_languages=target_languages,
            skip_existing=skip_existing,
            enabled=enabled,
        )
        self.session.add(library)
        self.session.flush()
        return library

    def get_by_id(self, id: str) -> Library | None:
        """Get a library by ID.

        Args:
            id: The library ID to find.

        Returns:
            The Library instance if found, None otherwise.
        """
        return self.session.query(Library).filter(Library.id == id).first()

    def list_all(self) -> list[Library]:
        """List all libraries.

        Returns:
            List of all Library instances.
        """
        return self.session.query(Library).all()

    def update(self, id: str, **kwargs: object) -> Library | None:
        """Update a library by ID.

        Args:
            id: The library ID to update.
            **kwargs: Fields to update.

        Returns:
            The updated Library instance if found, None otherwise.
        """
        library = self.get_by_id(id)
        if library is None:
            return None

        for key, value in kwargs.items():
            if hasattr(library, key):
                setattr(library, key, value)

        self.session.flush()
        return library

    def delete(self, id: str) -> bool:
        """Delete a library by ID.

        Args:
            id: The library ID to delete.

        Returns:
            True if deleted, False if not found.
        """
        library = self.get_by_id(id)
        if library is None:
            return False

        self.session.delete(library)
        self.session.flush()
        return True


class ItemRepository:
    """Repository for Item CRUD operations."""

    def __init__(self, session: Session) -> None:
        """Initialize repository with a database session.

        Args:
            session: SQLAlchemy session instance.
        """
        self.session = session

    def create(
        self,
        id: str,
        library_id: str,
        type: str,
        title: str,
        path: str,
        series_id: str | None = None,
        series_name: str | None = None,
        season_num: int | None = None,
        episode_num: int | None = None,
        poster_url: str | None = None,
    ) -> Item:
        """Create a new media item.

        Args:
            id: Jellyfin item ID.
            library_id: Parent library ID.
            type: Item type ('movie' or 'episode').
            title: Item title.
            path: File path to the media.
            series_id: Series ID for episodes.
            series_name: Series name for episodes.
            season_num: Season number for episodes.
            episode_num: Episode number for episodes.
            poster_url: URL to poster image.

        Returns:
            The created Item instance.
        """
        item = Item(
            id=id,
            library_id=library_id,
            type=type,
            title=title,
            path=path,
            series_id=series_id,
            series_name=series_name,
            season_num=season_num,
            episode_num=episode_num,
            poster_url=poster_url,
        )
        self.session.add(item)
        self.session.flush()
        return item

    def get_by_id(self, id: str) -> Item | None:
        """Get an item by ID.

        Args:
            id: The item ID to find.

        Returns:
            The Item instance if found, None otherwise.
        """
        return self.session.query(Item).filter(Item.id == id).first()

    def list_by_library(self, library_id: str, limit: int = 50, offset: int = 0) -> list[Item]:
        """List items by library with pagination.

        Args:
            library_id: The library ID to filter by.
            limit: Maximum number of items to return.
            offset: Number of items to skip.

        Returns:
            List of Item instances.
        """
        return self.session.query(Item).filter(Item.library_id == library_id).offset(offset).limit(limit).all()

    def list_by_series(self, series_id: str) -> list[Item]:
        """List items by series ID.

        Args:
            series_id: The series ID to filter by.

        Returns:
            List of Item instances belonging to the series.
        """
        return self.session.query(Item).filter(Item.series_id == series_id).all()

    def count_by_library(self, library_id: str) -> int:
        """Count items in a library.

        Args:
            library_id: The library ID to count items for.

        Returns:
            Number of items in the library.
        """
        result = self.session.query(func.count(Item.id)).filter(Item.library_id == library_id).scalar()
        return result or 0

    def upsert(
        self,
        id: str,
        library_id: str,
        type: str,
        title: str,
        path: str,
        series_id: str | None = None,
        series_name: str | None = None,
        season_num: int | None = None,
        episode_num: int | None = None,
        poster_url: str | None = None,
    ) -> Item:
        """Insert or update an item.

        Args:
            id: Jellyfin item ID.
            library_id: Parent library ID.
            type: Item type ('movie' or 'episode').
            title: Item title.
            path: File path to the media.
            series_id: Series ID for episodes.
            series_name: Series name for episodes.
            season_num: Season number for episodes.
            episode_num: Episode number for episodes.
            poster_url: URL to poster image.

        Returns:
            The created or updated Item instance.
        """
        existing = self.get_by_id(id)
        if existing is not None:
            existing.library_id = library_id
            existing.type = type
            existing.title = title
            existing.path = path
            existing.series_id = series_id
            existing.series_name = series_name
            existing.season_num = season_num
            existing.episode_num = episode_num
            existing.poster_url = poster_url
            existing.last_synced = datetime.now(UTC)
            self.session.flush()
            return existing

        return self.create(
            id=id,
            library_id=library_id,
            type=type,
            title=title,
            path=path,
            series_id=series_id,
            series_name=series_name,
            season_num=season_num,
            episode_num=episode_num,
            poster_url=poster_url,
        )


class SubtitleRepository:
    """Repository for Subtitle CRUD operations."""

    def __init__(self, session: Session) -> None:
        """Initialize repository with a database session.

        Args:
            session: SQLAlchemy session instance.
        """
        self.session = session

    def create(
        self,
        item_id: str,
        language: str,
        source: str,
        path: str,
    ) -> Subtitle:
        """Create a new subtitle.

        Args:
            item_id: Parent item ID.
            language: Language code (e.g., 'en', 'es').
            source: Source type ('external' or 'generated').
            path: File path to the subtitle.

        Returns:
            The created Subtitle instance.
        """
        subtitle = Subtitle(
            item_id=item_id,
            language=language,
            source=source,
            path=path,
        )
        self.session.add(subtitle)
        self.session.flush()
        return subtitle

    def get_by_item_and_language(self, item_id: str, language: str) -> Subtitle | None:
        """Get a subtitle by item ID and language.

        Args:
            item_id: The item ID to find.
            language: The language code to find.

        Returns:
            The Subtitle instance if found, None otherwise.
        """
        return self.session.query(Subtitle).filter(Subtitle.item_id == item_id, Subtitle.language == language).first()

    def list_by_item(self, item_id: str) -> list[Subtitle]:
        """List all subtitles for an item.

        Args:
            item_id: The item ID to filter by.

        Returns:
            List of Subtitle instances.
        """
        return self.session.query(Subtitle).filter(Subtitle.item_id == item_id).all()

    def delete(self, id: int) -> bool:
        """Delete a subtitle by ID.

        Args:
            id: The subtitle ID to delete.

        Returns:
            True if deleted, False if not found.
        """
        subtitle = self.session.query(Subtitle).filter(Subtitle.id == id).first()
        if subtitle is None:
            return False

        self.session.delete(subtitle)
        self.session.flush()
        return True

    def upsert(
        self,
        item_id: str,
        language: str,
        source: str,
        path: str,
    ) -> Subtitle:
        """Insert or update a subtitle.

        Args:
            item_id: Parent item ID.
            language: Language code (e.g., 'en', 'es').
            source: Source type ('external' or 'generated').
            path: File path to the subtitle.

        Returns:
            The created or updated Subtitle instance.
        """
        existing = self.get_by_item_and_language(item_id, language)
        if existing is not None:
            existing.source = source
            existing.path = path
            self.session.flush()
            return existing

        return self.create(
            item_id=item_id,
            language=language,
            source=source,
            path=path,
        )


class JobRepository:
    """Repository for Job CRUD operations."""

    def __init__(self, session: Session) -> None:
        """Initialize repository with a database session.

        Args:
            session: SQLAlchemy session instance.
        """
        self.session = session

    def create(
        self,
        id: str,
        item_id: str,
        language: str,
        status: str = "pending",
    ) -> Job:
        """Create a new transcription job.

        Args:
            id: Job ID.
            item_id: Parent item ID.
            language: Target language code.
            status: Initial status (default: 'pending').

        Returns:
            The created Job instance.
        """
        job = Job(
            id=id,
            item_id=item_id,
            language=language,
            status=status,
        )
        self.session.add(job)
        self.session.flush()
        return job

    def get_by_id(self, id: str) -> Job | None:
        """Get a job by ID.

        Args:
            id: The job ID to find.

        Returns:
            The Job instance if found, None otherwise.
        """
        return self.session.query(Job).filter(Job.id == id).first()

    def list_by_status(self, status: str, limit: int = 50, offset: int = 0) -> list[Job]:
        """List jobs by status with pagination.

        Args:
            status: The status to filter by.
            limit: Maximum number of jobs to return.
            offset: Number of jobs to skip.

        Returns:
            List of Job instances.
        """
        return self.session.query(Job).filter(Job.status == status).offset(offset).limit(limit).all()

    def list_by_item(self, item_id: str) -> list[Job]:
        """List jobs by item ID.

        Args:
            item_id: The item ID to filter by.

        Returns:
            List of Job instances for the item.
        """
        return self.session.query(Job).filter(Job.item_id == item_id).all()

    def update_status(self, id: str, status: str, error: str | None = None) -> Job | None:
        """Update job status with appropriate timestamp handling.

        Sets started_at when status changes to 'running'.
        Sets completed_at when status changes to 'completed' or 'failed'.

        Args:
            id: The job ID to update.
            status: New status value.
            error: Error message (for 'failed' status).

        Returns:
            The updated Job instance if found, None otherwise.
        """
        job = self.get_by_id(id)
        if job is None:
            return None

        job.status = status

        if status == "running":
            job.started_at = datetime.now(UTC)
        elif status in ("completed", "failed"):
            job.completed_at = datetime.now(UTC)

        if error is not None:
            job.error = error

        self.session.flush()
        return job

    def count_by_status(self, status: str) -> int:
        """Count jobs by status.

        Args:
            status: The status to count.

        Returns:
            Number of jobs with the given status.
        """
        result = self.session.query(func.count(Job.id)).filter(Job.status == status).scalar()
        return result or 0
