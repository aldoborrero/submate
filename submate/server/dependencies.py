"""FastAPI dependency injection for Submate server.

Provides reusable dependencies for database sessions and repositories,
eliminating boilerplate code across route handlers.

Usage:
    from submate.server.dependencies import get_item_repo, get_job_repo

    @router.get("/items/{item_id}")
    async def get_item(
        item_id: str,
        item_repo: ItemRepository = Depends(get_item_repo),
    ):
        item = item_repo.get_by_id(item_id)
        ...
"""

from collections.abc import Generator
from pathlib import Path
from typing import Annotated

from fastapi import Depends
from sqlalchemy.orm import Session

from submate.config import get_config
from submate.database.repository import (
    ItemRepository,
    JobRepository,
    LibraryRepository,
    SubtitleRepository,
)
from submate.database.session import get_db_session


def get_db_path() -> Path:
    """Get database path from configuration.

    Returns:
        Path to the SQLite database file.
    """
    config = get_config()
    return Path(config.queue.db_path)


def get_session(db_path: Path = Depends(get_db_path)) -> Generator[Session]:
    """Yield a database session with automatic commit/rollback.

    This dependency provides a SQLAlchemy session that:
    - Auto-commits on successful completion
    - Rolls back on exception
    - Closes the session in all cases

    Args:
        db_path: Database path (injected by get_db_path dependency).

    Yields:
        SQLAlchemy Session instance.
    """
    with get_db_session(db_path) as session:
        yield session


# Type alias for cleaner function signatures
DbSession = Annotated[Session, Depends(get_session)]


def get_library_repo(session: DbSession) -> LibraryRepository:
    """Get LibraryRepository instance.

    Args:
        session: Database session (injected).

    Returns:
        LibraryRepository instance.
    """
    return LibraryRepository(session)


def get_item_repo(session: DbSession) -> ItemRepository:
    """Get ItemRepository instance.

    Args:
        session: Database session (injected).

    Returns:
        ItemRepository instance.
    """
    return ItemRepository(session)


def get_subtitle_repo(session: DbSession) -> SubtitleRepository:
    """Get SubtitleRepository instance.

    Args:
        session: Database session (injected).

    Returns:
        SubtitleRepository instance.
    """
    return SubtitleRepository(session)


def get_job_repo(session: DbSession) -> JobRepository:
    """Get JobRepository instance.

    Args:
        session: Database session (injected).

    Returns:
        JobRepository instance.
    """
    return JobRepository(session)


# Type aliases for cleaner function signatures
LibraryRepo = Annotated[LibraryRepository, Depends(get_library_repo)]
ItemRepo = Annotated[ItemRepository, Depends(get_item_repo)]
SubtitleRepo = Annotated[SubtitleRepository, Depends(get_subtitle_repo)]
JobRepo = Annotated[JobRepository, Depends(get_job_repo)]
