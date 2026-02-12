"""Library API router for Submate UI."""

import logging
from pathlib import Path

from fastapi import APIRouter, HTTPException

from submate.config import get_config
from submate.database.repository import ItemRepository, LibraryRepository
from submate.database.session import get_db_session
from submate.server.handlers.library.models import (
    LibraryListResponse,
    LibraryResponse,
    LibraryUpdateRequest,
)

logger = logging.getLogger(__name__)


def _get_db_path() -> Path:
    """Get database path from configuration.

    Returns:
        Path to the SQLite database file.
    """
    config = get_config()
    return Path(config.queue.db_path)


def create_library_router() -> APIRouter:
    """Create library API router.

    Returns:
        APIRouter with library endpoints.
    """
    router = APIRouter(prefix="/api/libraries", tags=["libraries"])

    @router.get("", response_model=LibraryListResponse)
    async def list_libraries() -> LibraryListResponse:
        """List all libraries with item counts."""
        db_path = _get_db_path()

        with get_db_session(db_path) as session:
            library_repo = LibraryRepository(session)
            item_repo = ItemRepository(session)

            libraries = library_repo.list_all()
            library_responses = []

            for library in libraries:
                item_count = item_repo.count_by_library(library.id)
                library_responses.append(
                    LibraryResponse(
                        id=library.id,
                        name=library.name,
                        type=library.type,
                        target_languages=library.target_languages,
                        skip_existing=library.skip_existing,
                        enabled=library.enabled,
                        last_synced=library.last_synced,
                        item_count=item_count,
                    )
                )

            return LibraryListResponse(
                libraries=library_responses,
                total=len(library_responses),
            )

    @router.get("/{library_id}", response_model=LibraryResponse)
    async def get_library(library_id: str) -> LibraryResponse:
        """Get a single library by ID.

        Args:
            library_id: The library ID to retrieve.

        Returns:
            LibraryResponse with library details.

        Raises:
            HTTPException: 404 if library not found.
        """
        db_path = _get_db_path()

        with get_db_session(db_path) as session:
            library_repo = LibraryRepository(session)
            item_repo = ItemRepository(session)

            library = library_repo.get_by_id(library_id)
            if library is None:
                raise HTTPException(status_code=404, detail="Library not found")

            item_count = item_repo.count_by_library(library.id)

            return LibraryResponse(
                id=library.id,
                name=library.name,
                type=library.type,
                target_languages=library.target_languages,
                skip_existing=library.skip_existing,
                enabled=library.enabled,
                last_synced=library.last_synced,
                item_count=item_count,
            )

    @router.patch("/{library_id}", response_model=LibraryResponse)
    async def update_library(library_id: str, update: LibraryUpdateRequest) -> LibraryResponse:
        """Update library settings.

        Args:
            library_id: The library ID to update.
            update: The fields to update.

        Returns:
            LibraryResponse with updated library details.

        Raises:
            HTTPException: 404 if library not found.
        """
        db_path = _get_db_path()

        with get_db_session(db_path) as session:
            library_repo = LibraryRepository(session)
            item_repo = ItemRepository(session)

            library = library_repo.get_by_id(library_id)
            if library is None:
                raise HTTPException(status_code=404, detail="Library not found")

            # Build update kwargs from non-None fields
            update_kwargs: dict[str, object] = {}
            if update.target_languages is not None:
                update_kwargs["target_languages"] = update.target_languages
            if update.skip_existing is not None:
                update_kwargs["skip_existing"] = update.skip_existing
            if update.enabled is not None:
                update_kwargs["enabled"] = update.enabled

            # Apply updates if any
            if update_kwargs:
                library = library_repo.update(library_id, **update_kwargs)
                if library is None:
                    raise HTTPException(status_code=404, detail="Library not found")

            item_count = item_repo.count_by_library(library.id)

            return LibraryResponse(
                id=library.id,
                name=library.name,
                type=library.type,
                target_languages=library.target_languages,
                skip_existing=library.skip_existing,
                enabled=library.enabled,
                last_synced=library.last_synced,
                item_count=item_count,
            )

    return router
