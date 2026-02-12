"""Items API router for Submate UI."""

import logging
from pathlib import Path

import httpx
from fastapi import APIRouter, HTTPException
from fastapi.responses import StreamingResponse

from submate.config import get_config
from submate.database.models import Item
from submate.database.repository import ItemRepository, SubtitleRepository
from submate.database.session import get_db_session
from submate.server.handlers.items.models import (
    ItemListResponse,
    ItemResponse,
    SeriesDetailResponse,
)

logger = logging.getLogger(__name__)


def _get_db_path() -> Path:
    """Get database path from configuration.

    Returns:
        Path to the SQLite database file.
    """
    config = get_config()
    return Path(config.queue.db_path)


def _item_to_response(item: Item, subtitle_languages: list[str]) -> ItemResponse:
    """Convert a database Item to an ItemResponse.

    Args:
        item: The database Item model.
        subtitle_languages: List of language codes for existing subtitles.

    Returns:
        ItemResponse with item details.
    """
    return ItemResponse(
        id=item.id,
        library_id=item.library_id,
        type=item.type,
        title=item.title,
        path=item.path,
        series_id=item.series_id,
        series_name=item.series_name,
        season_num=item.season_num,
        episode_num=item.episode_num,
        poster_url=item.poster_url,
        last_synced=item.last_synced,
        subtitle_languages=subtitle_languages,
    )


def _get_subtitle_languages(subtitle_repo: SubtitleRepository, item_id: str) -> list[str]:
    """Get list of subtitle languages for an item.

    Args:
        subtitle_repo: SubtitleRepository instance.
        item_id: The item ID to get subtitles for.

    Returns:
        List of language codes.
    """
    subtitles = subtitle_repo.list_by_item(item_id)
    return [sub.language for sub in subtitles]


def create_items_router() -> APIRouter:
    """Create items API router.

    Returns:
        APIRouter with items endpoints.
    """
    router = APIRouter(prefix="/api", tags=["items"])

    @router.get("/movies", response_model=ItemListResponse)
    async def list_movies(
        page: int = 1,
        page_size: int = 50,
        library_id: str | None = None,
    ) -> ItemListResponse:
        """List movies with pagination.

        Args:
            page: Page number (1-indexed).
            page_size: Number of items per page.
            library_id: Optional library ID to filter by.

        Returns:
            ItemListResponse with paginated movies.
        """
        db_path = _get_db_path()
        offset = (page - 1) * page_size

        with get_db_session(db_path) as session:
            subtitle_repo = SubtitleRepository(session)

            # Query movies
            query = session.query(Item).filter(Item.type == "movie")
            if library_id:
                query = query.filter(Item.library_id == library_id)

            # Get total count
            total = query.count()

            # Get paginated items
            items = query.offset(offset).limit(page_size).all()

            # Convert to responses with subtitle languages
            item_responses = []
            for item in items:
                subtitle_languages = _get_subtitle_languages(subtitle_repo, item.id)
                item_responses.append(_item_to_response(item, subtitle_languages))

            return ItemListResponse(
                items=item_responses,
                total=total,
                page=page,
                page_size=page_size,
            )

    @router.get("/series", response_model=ItemListResponse)
    async def list_series(
        page: int = 1,
        page_size: int = 50,
        library_id: str | None = None,
    ) -> ItemListResponse:
        """List series with pagination.

        Args:
            page: Page number (1-indexed).
            page_size: Number of items per page.
            library_id: Optional library ID to filter by.

        Returns:
            ItemListResponse with paginated series.
        """
        db_path = _get_db_path()
        offset = (page - 1) * page_size

        with get_db_session(db_path) as session:
            subtitle_repo = SubtitleRepository(session)

            # Query series (type='series', not episodes)
            query = session.query(Item).filter(Item.type == "series")
            if library_id:
                query = query.filter(Item.library_id == library_id)

            # Get total count
            total = query.count()

            # Get paginated items
            items = query.offset(offset).limit(page_size).all()

            # Convert to responses with subtitle languages
            item_responses = []
            for item in items:
                subtitle_languages = _get_subtitle_languages(subtitle_repo, item.id)
                item_responses.append(_item_to_response(item, subtitle_languages))

            return ItemListResponse(
                items=item_responses,
                total=total,
                page=page,
                page_size=page_size,
            )

    @router.get("/series/{series_id}", response_model=SeriesDetailResponse)
    async def get_series_detail(series_id: str) -> SeriesDetailResponse:
        """Get series detail with episodes.

        Args:
            series_id: The series ID to retrieve.

        Returns:
            SeriesDetailResponse with series details and episodes.

        Raises:
            HTTPException: 404 if series not found.
        """
        db_path = _get_db_path()

        with get_db_session(db_path) as session:
            item_repo = ItemRepository(session)
            subtitle_repo = SubtitleRepository(session)

            # Get the series
            series = item_repo.get_by_id(series_id)
            if series is None or series.type != "series":
                raise HTTPException(status_code=404, detail="Series not found")

            # Get episodes for this series
            episodes = item_repo.list_by_series(series_id)

            # Convert episodes to responses with subtitle languages
            episode_responses = []
            for episode in episodes:
                subtitle_languages = _get_subtitle_languages(subtitle_repo, episode.id)
                episode_responses.append(_item_to_response(episode, subtitle_languages))

            # Calculate season count
            seasons = set()
            for episode in episodes:
                if episode.season_num is not None:
                    seasons.add(episode.season_num)

            # Get subtitle languages for the series itself
            series_subtitle_languages = _get_subtitle_languages(subtitle_repo, series.id)

            return SeriesDetailResponse(
                id=series.id,
                library_id=series.library_id,
                type=series.type,
                title=series.title,
                path=series.path,
                series_id=series.series_id,
                series_name=series.series_name,
                season_num=series.season_num,
                episode_num=series.episode_num,
                poster_url=series.poster_url,
                last_synced=series.last_synced,
                subtitle_languages=series_subtitle_languages,
                episodes=episode_responses,
                season_count=len(seasons),
                episode_count=len(episodes),
            )

    @router.get("/items/{item_id}", response_model=ItemResponse)
    async def get_item(item_id: str) -> ItemResponse:
        """Get a single item by ID.

        Args:
            item_id: The item ID to retrieve.

        Returns:
            ItemResponse with item details.

        Raises:
            HTTPException: 404 if item not found.
        """
        db_path = _get_db_path()

        with get_db_session(db_path) as session:
            item_repo = ItemRepository(session)
            subtitle_repo = SubtitleRepository(session)

            item = item_repo.get_by_id(item_id)
            if item is None:
                raise HTTPException(status_code=404, detail="Item not found")

            subtitle_languages = _get_subtitle_languages(subtitle_repo, item.id)

            return _item_to_response(item, subtitle_languages)

    @router.get("/items/{item_id}/poster")
    async def get_item_poster(item_id: str) -> StreamingResponse:
        """Proxy poster image from Jellyfin.

        Args:
            item_id: The item ID to get poster for.

        Returns:
            StreamingResponse with the poster image.

        Raises:
            HTTPException: 404 if item not found or no poster available.
            HTTPException: 502 if Jellyfin server is unavailable.
        """
        db_path = _get_db_path()
        config = get_config()

        with get_db_session(db_path) as session:
            item_repo = ItemRepository(session)

            item = item_repo.get_by_id(item_id)
            if item is None:
                raise HTTPException(status_code=404, detail="Item not found")

            if not item.poster_url:
                raise HTTPException(status_code=404, detail="No poster available")

        # Build Jellyfin URL
        jellyfin_url = config.jellyfin.server_url
        if not jellyfin_url:
            raise HTTPException(status_code=502, detail="Jellyfin server URL not configured")

        poster_url = f"{jellyfin_url.rstrip('/')}{item.poster_url}"

        # Fetch poster from Jellyfin
        try:
            async with httpx.AsyncClient() as client:
                headers = {}
                if config.jellyfin.api_key:
                    headers["X-Emby-Token"] = config.jellyfin.api_key

                response = await client.get(poster_url, headers=headers)
                response.raise_for_status()

                # Stream the response
                content_type = response.headers.get("content-type", "image/jpeg")
                return StreamingResponse(
                    iter([response.content]),
                    media_type=content_type,
                )
        except httpx.HTTPError as e:
            logger.error("Failed to fetch poster from Jellyfin: %s", e)
            raise HTTPException(status_code=502, detail="Failed to fetch poster from Jellyfin")

    return router
