"""Library API router for Submate UI."""

import logging

from fastapi import APIRouter, HTTPException

from submate.database.models import Library
from submate.server.dependencies import ItemRepo, LibraryRepo
from submate.server.handlers.library.models import (
    LibraryListResponse,
    LibraryResponse,
    LibraryUpdateRequest,
)

logger = logging.getLogger(__name__)


def _library_to_response(library: Library, item_count: int) -> LibraryResponse:
    """Convert a database Library to a LibraryResponse.

    Args:
        library: The database Library model.
        item_count: Number of items in the library.

    Returns:
        LibraryResponse with library details.
    """
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
        library_responses = [
            _library_to_response(library, item_repo.count_by_library(library.id))
            for library in libraries
        ]

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

        return _library_to_response(library, item_repo.count_by_library(library.id))

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

        # Use Pydantic's exclude_unset to get only provided fields
        update_data = update.model_dump(exclude_unset=True)

        # Apply updates if any
        if update_data:
            library = library_repo.update(library_id, **update_data)
            if library is None:
                raise HTTPException(status_code=404, detail="Library not found")

        return _library_to_response(library, item_repo.count_by_library(library.id))

    return router
