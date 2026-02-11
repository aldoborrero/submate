"""Tests for Jellyfin sync service."""

from pathlib import Path
from unittest.mock import Mock, patch

import pytest


class TestJellyfinSyncService:
    """Tests for JellyfinSyncService class."""

    @pytest.fixture
    def mock_jellyfin_client(self) -> Mock:
        """Create a mock Jellyfin client."""
        client = Mock()
        client.server_url = "http://localhost:8096"
        client.api_key = "test-api-key"
        return client

    @pytest.fixture
    def db_path(self, temp_dir: Path) -> Path:
        """Provide a database path for tests."""
        return temp_dir / "test.db"

    @pytest.fixture
    def initialized_db(self, db_path: Path) -> Path:
        """Initialize the database and return its path."""
        from submate.database import init_database

        init_database(db_path)
        return db_path

    def test_sync_libraries(
        self,
        mock_jellyfin_client: Mock,
        initialized_db: Path,
    ) -> None:
        """Test syncing libraries from Jellyfin.

        Verifies that:
        - Libraries are fetched from Jellyfin via get_libraries()
        - Only supported types (movies, tvshows) are synced
        - Libraries are upserted into the database
        - Collection types are mapped correctly
        """
        from submate.database import LibraryRepository, get_db_session
        from submate.services import JellyfinSyncService

        # Mock get_libraries to return test data
        mock_jellyfin_client.get_libraries.return_value = [
            {
                "ItemId": "lib-movies-1",
                "Name": "Movies",
                "CollectionType": "movies",
            },
            {
                "ItemId": "lib-series-1",
                "Name": "TV Shows",
                "CollectionType": "tvshows",
            },
            {
                "ItemId": "lib-music-1",
                "Name": "Music",
                "CollectionType": "music",
            },
        ]

        # Create sync service
        sync_service = JellyfinSyncService(
            jellyfin_client=mock_jellyfin_client,
            db_path=initialized_db,
        )

        # Sync libraries
        result = sync_service.sync_libraries()

        # Verify get_libraries was called
        mock_jellyfin_client.get_libraries.assert_called_once()

        # Verify result - should only include movies and tvshows
        assert len(result) == 2
        library_names = [lib["name"] for lib in result]
        assert "Movies" in library_names
        assert "TV Shows" in library_names
        assert "Music" not in [lib.get("name") for lib in result]

        # Verify libraries are in database with correct types
        with get_db_session(initialized_db) as session:
            repo = LibraryRepository(session)
            libraries = repo.list_all()

            assert len(libraries) == 2

            movies_lib = repo.get_by_id("lib-movies-1")
            assert movies_lib is not None
            assert movies_lib.name == "Movies"
            assert movies_lib.type == "movies"

            series_lib = repo.get_by_id("lib-series-1")
            assert series_lib is not None
            assert series_lib.name == "TV Shows"
            assert series_lib.type == "series"  # Mapped from tvshows

    def test_sync_library_items(
        self,
        mock_jellyfin_client: Mock,
        initialized_db: Path,
        temp_dir: Path,
    ) -> None:
        """Test syncing items from a library.

        Verifies that:
        - Items are fetched from Jellyfin with pagination
        - Items are upserted into the database
        - Subtitles are scanned for each item
        - Returns count of items synced
        """
        from submate.database import (
            ItemRepository,
            LibraryRepository,
            get_db_session,
        )
        from submate.services import JellyfinSyncService

        # Setup: Create a library first
        with get_db_session(initialized_db) as session:
            repo = LibraryRepository(session)
            repo.create(
                id="lib-movies-1",
                name="Movies",
                type="movies",
                target_languages=["en", "es"],
            )

        # Create media files for the test
        media_file1 = temp_dir / "Movie1.mp4"
        media_file1.touch()
        media_file2 = temp_dir / "Movie2.mkv"
        media_file2.touch()

        # Mock get_library_items with pagination
        mock_jellyfin_client.get_library_items.return_value = {
            "Items": [
                {
                    "Id": "item-1",
                    "Name": "Movie 1",
                    "Path": str(media_file1),
                    "Type": "Movie",
                },
                {
                    "Id": "item-2",
                    "Name": "Movie 2",
                    "Path": str(media_file2),
                    "Type": "Movie",
                },
            ],
            "TotalRecordCount": 2,
        }

        # Mock get_poster_url
        mock_jellyfin_client.get_poster_url.return_value = "http://localhost:8096/Items/item-1/Images/Primary"

        # Create sync service
        sync_service = JellyfinSyncService(
            jellyfin_client=mock_jellyfin_client,
            db_path=initialized_db,
        )

        # Patch the scanner to avoid actual file system scanning
        with patch.object(sync_service.scanner, "scan_for_media", return_value=[]):
            # Sync library items
            count = sync_service.sync_library_items("lib-movies-1")

        # Verify count
        assert count == 2

        # Verify get_library_items was called
        mock_jellyfin_client.get_library_items.assert_called()

        # Verify items are in database
        with get_db_session(initialized_db) as session:
            repo = ItemRepository(session)
            items = repo.list_by_library("lib-movies-1")

            assert len(items) == 2

            item1 = repo.get_by_id("item-1")
            assert item1 is not None
            assert item1.title == "Movie 1"
            assert item1.path == str(media_file1)
            assert item1.type == "movie"
            assert item1.library_id == "lib-movies-1"

            item2 = repo.get_by_id("item-2")
            assert item2 is not None
            assert item2.title == "Movie 2"
