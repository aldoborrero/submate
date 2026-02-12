"""Subtitles API router for Submate UI."""

import logging
from pathlib import Path

from fastapi import APIRouter, HTTPException, Response

from submate.config import get_config
from submate.database.models import Subtitle
from submate.database.repository import ItemRepository, SubtitleRepository
from submate.database.session import get_db_session
from submate.server.handlers.subtitles.models import (
    SubtitleContentResponse,
    SubtitleListResponse,
    SubtitleResponse,
    SubtitleUpdateRequest,
    SyncResponse,
)

logger = logging.getLogger(__name__)


def _get_db_path() -> Path:
    """Get database path from configuration.

    Returns:
        Path to the SQLite database file.
    """
    config = get_config()
    return Path(config.queue.db_path)


def _subtitle_to_response(subtitle: Subtitle) -> SubtitleResponse:
    """Convert a database Subtitle to a SubtitleResponse.

    Args:
        subtitle: The database Subtitle model.

    Returns:
        SubtitleResponse with subtitle details.
    """
    return SubtitleResponse(
        id=subtitle.id,
        item_id=subtitle.item_id,
        language=subtitle.language,
        source=subtitle.source,
        path=subtitle.path,
        created_at=subtitle.created_at,
    )


def _get_subtitle_format(path: str) -> str:
    """Detect subtitle format from file extension.

    Args:
        path: Path to the subtitle file.

    Returns:
        Format string ('srt', 'ass', 'vtt', or 'unknown').
    """
    suffix = Path(path).suffix.lower()
    format_map = {
        ".srt": "srt",
        ".ass": "ass",
        ".ssa": "ass",
        ".vtt": "vtt",
    }
    return format_map.get(suffix, "unknown")


def create_subtitles_router() -> APIRouter:
    """Create subtitles API router.

    Returns:
        APIRouter with subtitles endpoints.
    """
    router = APIRouter(prefix="/api", tags=["subtitles"])

    @router.get("/items/{item_id}/subtitles", response_model=SubtitleListResponse)
    async def list_subtitles(item_id: str) -> SubtitleListResponse:
        """List all subtitles for an item.

        Args:
            item_id: The item ID to list subtitles for.

        Returns:
            SubtitleListResponse with list of subtitles.

        Raises:
            HTTPException: 404 if item not found.
        """
        db_path = _get_db_path()

        with get_db_session(db_path) as session:
            item_repo = ItemRepository(session)
            subtitle_repo = SubtitleRepository(session)

            # Check if item exists
            item = item_repo.get_by_id(item_id)
            if item is None:
                raise HTTPException(status_code=404, detail="Item not found")

            # Get subtitles
            subtitles = subtitle_repo.list_by_item(item_id)

            return SubtitleListResponse(
                subtitles=[_subtitle_to_response(sub) for sub in subtitles],
                total=len(subtitles),
            )

    @router.get("/items/{item_id}/subtitles/{language}", response_model=SubtitleContentResponse)
    async def get_subtitle_content(item_id: str, language: str) -> SubtitleContentResponse:
        """Get subtitle content for an item.

        Args:
            item_id: The item ID to get subtitle for.
            language: The language code of the subtitle.

        Returns:
            SubtitleContentResponse with subtitle content.

        Raises:
            HTTPException: 404 if item or subtitle not found.
            HTTPException: 500 if file read fails.
        """
        db_path = _get_db_path()

        with get_db_session(db_path) as session:
            item_repo = ItemRepository(session)
            subtitle_repo = SubtitleRepository(session)

            # Check if item exists
            item = item_repo.get_by_id(item_id)
            if item is None:
                raise HTTPException(status_code=404, detail="Item not found")

            # Get subtitle
            subtitle = subtitle_repo.get_by_item_and_language(item_id, language)
            if subtitle is None:
                raise HTTPException(status_code=404, detail="Subtitle not found")

            # Read file content
            subtitle_path = Path(subtitle.path)
            try:
                content = subtitle_path.read_text(encoding="utf-8")
            except FileNotFoundError:
                raise HTTPException(status_code=404, detail="Subtitle file not found")
            except Exception as e:
                logger.error("Failed to read subtitle file %s: %s", subtitle.path, e)
                raise HTTPException(status_code=500, detail="Failed to read subtitle file")

            return SubtitleContentResponse(
                language=language,
                content=content,
                format=_get_subtitle_format(subtitle.path),
            )

    @router.put("/items/{item_id}/subtitles/{language}", response_model=SubtitleResponse)
    async def save_subtitle(
        item_id: str,
        language: str,
        request: SubtitleUpdateRequest,
    ) -> SubtitleResponse:
        """Save or update subtitle content.

        Args:
            item_id: The item ID to save subtitle for.
            language: The language code of the subtitle.
            request: Request body with content.

        Returns:
            SubtitleResponse with saved subtitle details.

        Raises:
            HTTPException: 404 if item not found.
            HTTPException: 500 if file write fails.
        """
        db_path = _get_db_path()

        with get_db_session(db_path) as session:
            item_repo = ItemRepository(session)
            subtitle_repo = SubtitleRepository(session)

            # Check if item exists
            item = item_repo.get_by_id(item_id)
            if item is None:
                raise HTTPException(status_code=404, detail="Item not found")

            # Check if subtitle already exists
            existing_subtitle = subtitle_repo.get_by_item_and_language(item_id, language)

            if existing_subtitle is not None:
                # Update existing subtitle
                subtitle_path = Path(existing_subtitle.path)
                try:
                    subtitle_path.write_text(request.content, encoding="utf-8")
                except Exception as e:
                    logger.error("Failed to write subtitle file %s: %s", existing_subtitle.path, e)
                    raise HTTPException(status_code=500, detail="Failed to write subtitle file")

                # Update source to 'generated' since it was edited
                subtitle = subtitle_repo.upsert(
                    item_id=item_id,
                    language=language,
                    source="generated",
                    path=existing_subtitle.path,
                )
            else:
                # Create new subtitle
                # Generate path based on item path
                item_path = Path(item.path)
                subtitle_path = item_path.with_suffix(f".{language}.srt")

                try:
                    subtitle_path.write_text(request.content, encoding="utf-8")
                except Exception as e:
                    logger.error("Failed to write subtitle file %s: %s", subtitle_path, e)
                    raise HTTPException(status_code=500, detail="Failed to write subtitle file")

                subtitle = subtitle_repo.create(
                    item_id=item_id,
                    language=language,
                    source="generated",
                    path=str(subtitle_path),
                )

            return _subtitle_to_response(subtitle)

    @router.delete("/items/{item_id}/subtitles/{language}", status_code=204)
    async def delete_subtitle(item_id: str, language: str) -> Response:
        """Delete a subtitle.

        Args:
            item_id: The item ID to delete subtitle for.
            language: The language code of the subtitle.

        Returns:
            Empty response with 204 status.

        Raises:
            HTTPException: 404 if item or subtitle not found.
        """
        db_path = _get_db_path()

        with get_db_session(db_path) as session:
            item_repo = ItemRepository(session)
            subtitle_repo = SubtitleRepository(session)

            # Check if item exists
            item = item_repo.get_by_id(item_id)
            if item is None:
                raise HTTPException(status_code=404, detail="Item not found")

            # Get subtitle
            subtitle = subtitle_repo.get_by_item_and_language(item_id, language)
            if subtitle is None:
                raise HTTPException(status_code=404, detail="Subtitle not found")

            # Delete file from disk
            subtitle_path = Path(subtitle.path)
            try:
                if subtitle_path.exists():
                    subtitle_path.unlink()
            except Exception as e:
                logger.error("Failed to delete subtitle file %s: %s", subtitle.path, e)
                # Continue to delete database record even if file deletion fails

            # Delete from database
            subtitle_repo.delete(subtitle.id)

            return Response(status_code=204)

    @router.post("/items/{item_id}/subtitles/{language}/sync", response_model=SyncResponse)
    async def sync_subtitle(item_id: str, language: str) -> SyncResponse:
        """Sync subtitle timing with ffsubsync.

        Note: This is currently a stub implementation.
        Full ffsubsync integration will be added in a future release.

        Args:
            item_id: The item ID to sync subtitle for.
            language: The language code of the subtitle.

        Returns:
            SyncResponse with operation result.

        Raises:
            HTTPException: 404 if item or subtitle not found.
        """
        db_path = _get_db_path()

        with get_db_session(db_path) as session:
            item_repo = ItemRepository(session)
            subtitle_repo = SubtitleRepository(session)

            # Check if item exists
            item = item_repo.get_by_id(item_id)
            if item is None:
                raise HTTPException(status_code=404, detail="Item not found")

            # Check if subtitle exists
            subtitle = subtitle_repo.get_by_item_and_language(item_id, language)
            if subtitle is None:
                raise HTTPException(status_code=404, detail="Subtitle not found")

            # TODO: Implement actual ffsubsync integration
            # For now, return a stub response
            return SyncResponse(
                success=True,
                message="Subtitle sync is not yet implemented. This is a stub response.",
            )

    return router
