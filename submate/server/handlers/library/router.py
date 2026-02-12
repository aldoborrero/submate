"""Library API router for Submate UI."""

import logging

from fastapi import APIRouter, HTTPException

from submate.server.dependencies import ItemRepo, LibraryRepo
from submate.server.handlers.library.models import (
    LibraryListResponse,
    LibraryResponse,
    LibraryUpdateRequest,
)

logger = logging.getLogger(__name__)


def create_library_router() -> APIRouter:
    """Create library API router.

    Returns:
        APIRouter with library endpoints.
    """
    router = APIRouter(prefix="/api/libraries", tags=["libraries"])

    @router.get("", response_model=LibraryListResponse)
    async def list_libraries(
        library_repo: LibraryRepo,
        item_repo: ItemRepo,
    ) -> LibraryListResponse:
        """List all libraries with item counts."""
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
    async def get_library(
        library_id: str,
        library_repo: LibraryRepo,
        item_repo: ItemRepo,
    ) -> LibraryResponse:
        """Get a single library by ID.

        Raises:
            HTTPException: 404 if library not found.
        """
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
    async def update_library(
        library_id: str,
        update: LibraryUpdateRequest,
        library_repo: LibraryRepo,
        item_repo: ItemRepo,
    ) -> LibraryResponse:
        """Update library settings.

        Raises:
            HTTPException: 404 if library not found.
        """
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
