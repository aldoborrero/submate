"""Jellyfin sync service for synchronizing libraries and items.

Provides functionality to sync Jellyfin libraries and media items
to the local database, including subtitle scanning.
"""

import logging
from datetime import UTC, datetime
from pathlib import Path
from typing import TYPE_CHECKING, Any

from submate.database import (
    ItemRepository,
    LibraryRepository,
    SubtitleRepository,
    get_db_session,
    init_database,
)
from submate.services.event_bus import EventBus, get_event_bus
from submate.services.scanner import SubtitleScanner

if TYPE_CHECKING:
    from submate.media_servers.jellyfin import JellyfinClient

logger = logging.getLogger(__name__)

# Supported Jellyfin collection types
SUPPORTED_COLLECTION_TYPES = {"movies", "tvshows"}

# Mapping from Jellyfin collection types to internal types
COLLECTION_TYPE_MAP: dict[str, str] = {
    "movies": "movies",
    "tvshows": "series",
}


class JellyfinSyncService:
    """Service for syncing Jellyfin libraries and items to the database.

    This service handles:
    - Syncing library metadata from Jellyfin
    - Syncing media items (movies, episodes) from libraries
    - Scanning for existing subtitle files
    - Publishing sync events via the event bus
    """

    def __init__(
        self,
        jellyfin_client: "JellyfinClient",
        db_path: Path,
    ) -> None:
        """Initialize the sync service.

        Args:
            jellyfin_client: Jellyfin API client instance.
            db_path: Path to the SQLite database file.
        """
        self.jellyfin_client = jellyfin_client
        self.db_path = db_path
        self.scanner = SubtitleScanner()
        self.event_bus: EventBus = get_event_bus()

        # Ensure database is initialized
        init_database(db_path)

    def _map_collection_type(self, collection_type: str) -> str | None:
        """Map Jellyfin collection type to internal type.

        Args:
            collection_type: Jellyfin collection type (e.g., 'movies', 'tvshows').

        Returns:
            Internal type ('movies', 'series') or None if not supported.
        """
        return COLLECTION_TYPE_MAP.get(collection_type)

    def sync_libraries(self) -> list[dict[str, Any]]:
        """Sync libraries from Jellyfin to the database.

        Fetches all libraries from Jellyfin and upserts supported ones
        (movies, tvshows) into the database.

        Returns:
            List of synced library info dicts with keys: id, name, type.
        """
        logger.info("Syncing libraries from Jellyfin")

        # Fetch libraries from Jellyfin
        jellyfin_libraries = self.jellyfin_client.get_libraries()

        synced_libraries: list[dict[str, Any]] = []

        with get_db_session(self.db_path) as session:
            repo = LibraryRepository(session)

            for jf_lib in jellyfin_libraries:
                collection_type = jf_lib.get("CollectionType", "")
                library_id = jf_lib.get("ItemId", "")
                library_name = jf_lib.get("Name", "")

                # Skip unsupported collection types
                internal_type = self._map_collection_type(collection_type)
                if internal_type is None:
                    logger.debug(
                        "Skipping unsupported library type: %s (%s)",
                        library_name,
                        collection_type,
                    )
                    continue

                # Upsert library
                existing = repo.get_by_id(library_id)
                if existing is not None:
                    # Update existing library
                    repo.update(
                        library_id,
                        name=library_name,
                        type=internal_type,
                        last_synced=datetime.now(UTC),
                    )
                    logger.debug("Updated library: %s", library_name)
                else:
                    # Create new library
                    repo.create(
                        id=library_id,
                        name=library_name,
                        type=internal_type,
                        target_languages=["en"],  # Default target language
                    )
                    logger.debug("Created library: %s", library_name)

                synced_libraries.append(
                    {
                        "id": library_id,
                        "name": library_name,
                        "type": internal_type,
                    }
                )

        logger.info("Synced %d libraries from Jellyfin", len(synced_libraries))
        return synced_libraries

    def sync_library_items(self, library_id: str) -> int:
        """Sync items from a library to the database.

        Fetches all items from the specified library with pagination,
        upserts them into the database, and scans for subtitle files.

        For series libraries, this also syncs all episodes.

        Args:
            library_id: The Jellyfin library ID to sync items from.

        Returns:
            Number of items synced.
        """
        logger.info("Syncing items for library: %s", library_id)

        # Get library info to determine type
        with get_db_session(self.db_path) as session:
            lib_repo = LibraryRepository(session)
            library = lib_repo.get_by_id(library_id)

            if library is None:
                logger.warning("Library not found in database: %s", library_id)
                return 0

            library_type = library.type

        # Determine item type based on library type
        if library_type == "movies":
            item_type = "Movie"
        elif library_type == "series":
            item_type = "Series"
        else:
            logger.warning("Unknown library type: %s", library_type)
            return 0

        items_synced = 0
        start_index = 0
        page_size = 100

        # Fetch items with pagination
        while True:
            response = self.jellyfin_client.get_library_items(
                library_id=library_id,
                item_type=item_type,
                start_index=start_index,
                limit=page_size,
            )

            items = response.get("Items", [])
            total_count = response.get("TotalRecordCount", 0)

            if not items:
                break

            for item in items:
                self._sync_item(library_id, library_type, item)
                items_synced += 1

            start_index += len(items)

            # Check if we've fetched all items
            if start_index >= total_count:
                break

        # For series, also sync episodes
        if library_type == "series":
            episodes_synced = self._sync_all_episodes(library_id)
            items_synced += episodes_synced

        # Update library last_synced timestamp
        with get_db_session(self.db_path) as session:
            lib_repo = LibraryRepository(session)
            lib_repo.update(library_id, last_synced=datetime.now(UTC))

        logger.info("Synced %d items for library: %s", items_synced, library_id)
        return items_synced

    def _sync_item(
        self,
        library_id: str,
        library_type: str,
        item: dict[str, Any],
        series_id: str | None = None,
        series_name: str | None = None,
    ) -> None:
        """Sync a single item to the database.

        Args:
            library_id: The parent library ID.
            library_type: The library type ('movies' or 'series').
            item: Jellyfin item data dict.
            series_id: Optional series ID (for episodes).
            series_name: Optional series name (for episodes).
        """
        item_id = item.get("Id", "")
        item_name = item.get("Name", "")
        item_path = item.get("Path", "")
        item_type_jf = item.get("Type", "")

        # Determine internal item type
        if item_type_jf == "Movie":
            internal_type = "movie"
        elif item_type_jf == "Episode":
            internal_type = "episode"
        elif item_type_jf == "Series":
            # Series entries are just for navigation, not actual media
            return
        else:
            logger.debug("Skipping unsupported item type: %s", item_type_jf)
            return

        # Get poster URL
        try:
            poster_url = self.jellyfin_client.get_poster_url(item_id)
        except Exception:
            poster_url = None

        # Extract episode info if applicable
        season_num = item.get("ParentIndexNumber")
        episode_num = item.get("IndexNumber")

        # Upsert item to database
        with get_db_session(self.db_path) as session:
            item_repo = ItemRepository(session)
            item_repo.upsert(
                id=item_id,
                library_id=library_id,
                type=internal_type,
                title=item_name,
                path=item_path,
                series_id=series_id,
                series_name=series_name,
                season_num=season_num,
                episode_num=episode_num,
                poster_url=poster_url,
            )

        # Scan for subtitles
        if item_path:
            self._scan_and_save_subtitles(item_id, Path(item_path))

    def _scan_and_save_subtitles(self, item_id: str, media_path: Path) -> None:
        """Scan for subtitles and save them to the database.

        Args:
            item_id: The media item ID.
            media_path: Path to the media file.
        """
        subtitles = self.scanner.scan_for_media(media_path)

        if not subtitles:
            return

        with get_db_session(self.db_path) as session:
            sub_repo = SubtitleRepository(session)

            for sub_info in subtitles:
                sub_repo.upsert(
                    item_id=item_id,
                    language=sub_info["language"],
                    source=sub_info["source"],
                    path=str(sub_info["path"]),
                )

    def _sync_all_episodes(self, library_id: str) -> int:
        """Sync all episodes for series in a library.

        Args:
            library_id: The series library ID.

        Returns:
            Number of episodes synced.
        """
        episodes_synced = 0

        # We need to fetch series items directly from Jellyfin
        # since we don't store series entries in our items table

        # Fetch series from Jellyfin
        start_index = 0
        page_size = 100

        while True:
            response = self.jellyfin_client.get_library_items(
                library_id=library_id,
                item_type="Series",
                start_index=start_index,
                limit=page_size,
            )

            series_items = response.get("Items", [])
            total_count = response.get("TotalRecordCount", 0)

            if not series_items:
                break

            for series in series_items:
                series_id = series.get("Id", "")
                series_name = series.get("Name", "")

                episodes = self._sync_series_episodes(library_id, series_id, series_name)
                episodes_synced += episodes

            start_index += len(series_items)

            if start_index >= total_count:
                break

        return episodes_synced

    def _sync_series_episodes(self, library_id: str, series_id: str, series_name: str) -> int:
        """Sync all episodes for a specific series.

        Args:
            library_id: The parent library ID.
            series_id: The series ID.
            series_name: The series name.

        Returns:
            Number of episodes synced.
        """
        logger.debug("Syncing episodes for series: %s", series_name)

        try:
            response = self.jellyfin_client.get_series_episodes(series_id)
        except Exception as e:
            logger.error("Failed to fetch episodes for %s: %s", series_name, e)
            return 0

        episodes = response.get("Items", [])
        episodes_synced = 0

        for episode in episodes:
            self._sync_item(
                library_id=library_id,
                library_type="series",
                item=episode,
                series_id=series_id,
                series_name=series_name,
            )
            episodes_synced += 1

        return episodes_synced

    def sync_all(self) -> dict[str, Any]:
        """Sync all libraries and their items from Jellyfin.

        This is the main entry point for a full sync operation.
        It syncs libraries first, then items for each library.

        Returns:
            Summary dict with keys: libraries, items, timestamp.
        """
        logger.info("Starting full Jellyfin sync")

        # Sync libraries
        synced_libraries = self.sync_libraries()

        # Sync items for each library
        total_items = 0
        for lib_info in synced_libraries:
            items_count = self.sync_library_items(lib_info["id"])
            total_items += items_count

        summary = {
            "libraries": len(synced_libraries),
            "items": total_items,
            "timestamp": datetime.now(UTC).isoformat(),
        }

        # Publish sync completed event
        self.event_bus.publish(
            "sync.completed",
            {
                "libraries": synced_libraries,
                "total_items": total_items,
                "timestamp": summary["timestamp"],
            },
        )

        logger.info(
            "Full sync completed: %d libraries, %d items",
            len(synced_libraries),
            total_items,
        )

        return summary
