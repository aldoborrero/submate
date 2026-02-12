# tests/test_database_repository.py
"""Tests for database repository layer."""

from pathlib import Path

import pytest


@pytest.fixture
def db_path(tmp_path: Path) -> Path:
    """Create an initialized test database."""
    from submate.database import init_database

    db_path = tmp_path / "test.db"
    init_database(db_path)
    return db_path


def test_create_library(db_path: Path) -> None:
    """Test creating a library through the repository."""
    from submate.database import LibraryRepository, get_db_session

    with get_db_session(db_path) as session:
        repo = LibraryRepository(session)
        library = repo.create(
            id="lib-123",
            name="Movies",
            type="movies",
            target_languages=["en", "es"],
            skip_existing=True,
            enabled=True,
        )

        assert library.id == "lib-123"
        assert library.name == "Movies"
        assert library.type == "movies"
        assert library.target_languages == ["en", "es"]
        assert library.skip_existing is True
        assert library.enabled is True


def test_get_library_by_id(db_path: Path) -> None:
    """Test getting a library by ID."""
    from submate.database import LibraryRepository, get_db_session

    # Create a library first
    with get_db_session(db_path) as session:
        repo = LibraryRepository(session)
        repo.create(
            id="lib-456",
            name="TV Shows",
            type="series",
            target_languages=["en"],
        )

    # Retrieve in a new session
    with get_db_session(db_path) as session:
        repo = LibraryRepository(session)
        library = repo.get_by_id("lib-456")

        assert library is not None
        assert library.id == "lib-456"
        assert library.name == "TV Shows"
        assert library.type == "series"

        # Test non-existent library
        not_found = repo.get_by_id("nonexistent")
        assert not_found is None


def test_list_all_libraries(db_path: Path) -> None:
    """Test listing all libraries."""
    from submate.database import LibraryRepository, get_db_session

    with get_db_session(db_path) as session:
        repo = LibraryRepository(session)
        repo.create(id="lib-1", name="Movies", type="movies", target_languages=["en"])
        repo.create(id="lib-2", name="TV Shows", type="series", target_languages=["es"])

    with get_db_session(db_path) as session:
        repo = LibraryRepository(session)
        libraries = repo.list_all()

        assert len(libraries) == 2
        names = [lib.name for lib in libraries]
        assert "Movies" in names
        assert "TV Shows" in names


def test_update_library(db_path: Path) -> None:
    """Test updating a library."""
    from submate.database import LibraryRepository, get_db_session

    with get_db_session(db_path) as session:
        repo = LibraryRepository(session)
        repo.create(id="lib-upd", name="Old Name", type="movies", target_languages=["en"])

    with get_db_session(db_path) as session:
        repo = LibraryRepository(session)
        updated = repo.update("lib-upd", name="New Name", enabled=False)

        assert updated is not None
        assert updated.name == "New Name"
        assert updated.enabled is False

    # Verify persistence
    with get_db_session(db_path) as session:
        repo = LibraryRepository(session)
        library = repo.get_by_id("lib-upd")
        assert library is not None
        assert library.name == "New Name"


def test_delete_library(db_path: Path) -> None:
    """Test deleting a library."""
    from submate.database import LibraryRepository, get_db_session

    with get_db_session(db_path) as session:
        repo = LibraryRepository(session)
        repo.create(id="lib-del", name="To Delete", type="movies", target_languages=["en"])

    with get_db_session(db_path) as session:
        repo = LibraryRepository(session)
        result = repo.delete("lib-del")
        assert result is True

        # Should not exist anymore
        library = repo.get_by_id("lib-del")
        assert library is None

        # Deleting non-existent returns False
        result = repo.delete("nonexistent")
        assert result is False


def test_create_item(db_path: Path) -> None:
    """Test creating an item through the repository."""
    from submate.database import ItemRepository, LibraryRepository, get_db_session

    # First create a library
    with get_db_session(db_path) as session:
        lib_repo = LibraryRepository(session)
        lib_repo.create(id="lib-items", name="Movies", type="movies", target_languages=["en"])

    # Create an item
    with get_db_session(db_path) as session:
        repo = ItemRepository(session)
        item = repo.create(
            id="item-123",
            library_id="lib-items",
            type="movie",
            title="Test Movie",
            path="/media/movies/test.mkv",
            poster_url="http://example.com/poster.jpg",
        )

        assert item.id == "item-123"
        assert item.library_id == "lib-items"
        assert item.type == "movie"
        assert item.title == "Test Movie"
        assert item.path == "/media/movies/test.mkv"
        assert item.poster_url == "http://example.com/poster.jpg"


def test_create_episode_item(db_path: Path) -> None:
    """Test creating an episode item with series metadata."""
    from submate.database import ItemRepository, LibraryRepository, get_db_session

    with get_db_session(db_path) as session:
        lib_repo = LibraryRepository(session)
        lib_repo.create(id="lib-tv", name="TV Shows", type="series", target_languages=["en"])

    with get_db_session(db_path) as session:
        repo = ItemRepository(session)
        item = repo.create(
            id="ep-001",
            library_id="lib-tv",
            type="episode",
            title="Pilot",
            path="/media/tv/show/s01e01.mkv",
            series_id="series-123",
            series_name="Test Show",
            season_num=1,
            episode_num=1,
        )

        assert item.series_id == "series-123"
        assert item.series_name == "Test Show"
        assert item.season_num == 1
        assert item.episode_num == 1


def test_list_items_by_library(db_path: Path) -> None:
    """Test listing items by library with pagination."""
    from submate.database import ItemRepository, LibraryRepository, get_db_session

    # Create library and items
    with get_db_session(db_path) as session:
        lib_repo = LibraryRepository(session)
        lib_repo.create(id="lib-list", name="Movies", type="movies", target_languages=["en"])

        item_repo = ItemRepository(session)
        for i in range(5):
            item_repo.create(
                id=f"item-{i}",
                library_id="lib-list",
                type="movie",
                title=f"Movie {i}",
                path=f"/media/movies/movie{i}.mkv",
            )

    # Test listing with pagination
    with get_db_session(db_path) as session:
        repo = ItemRepository(session)

        # Get first page
        items = repo.list_by_library("lib-list", limit=3, offset=0)
        assert len(items) == 3

        # Get second page
        items = repo.list_by_library("lib-list", limit=3, offset=3)
        assert len(items) == 2

        # Get all
        items = repo.list_by_library("lib-list")
        assert len(items) == 5


def test_list_items_by_series(db_path: Path) -> None:
    """Test listing items by series ID."""
    from submate.database import ItemRepository, LibraryRepository, get_db_session

    with get_db_session(db_path) as session:
        lib_repo = LibraryRepository(session)
        lib_repo.create(id="lib-series", name="TV Shows", type="series", target_languages=["en"])

        item_repo = ItemRepository(session)
        for i in range(3):
            item_repo.create(
                id=f"ep-{i}",
                library_id="lib-series",
                type="episode",
                title=f"Episode {i}",
                path=f"/media/tv/show/s01e0{i}.mkv",
                series_id="series-abc",
                series_name="Test Show",
                season_num=1,
                episode_num=i + 1,
            )
        # Add item from different series
        item_repo.create(
            id="ep-other",
            library_id="lib-series",
            type="episode",
            title="Other Episode",
            path="/media/tv/other/s01e01.mkv",
            series_id="series-xyz",
            series_name="Other Show",
            season_num=1,
            episode_num=1,
        )

    with get_db_session(db_path) as session:
        repo = ItemRepository(session)
        items = repo.list_by_series("series-abc")
        assert len(items) == 3
        for item in items:
            assert item.series_id == "series-abc"


def test_count_items_by_library(db_path: Path) -> None:
    """Test counting items in a library."""
    from submate.database import ItemRepository, LibraryRepository, get_db_session

    with get_db_session(db_path) as session:
        lib_repo = LibraryRepository(session)
        lib_repo.create(id="lib-count", name="Movies", type="movies", target_languages=["en"])

        item_repo = ItemRepository(session)
        for i in range(7):
            item_repo.create(
                id=f"cnt-{i}",
                library_id="lib-count",
                type="movie",
                title=f"Movie {i}",
                path=f"/media/movies/cnt{i}.mkv",
            )

    with get_db_session(db_path) as session:
        repo = ItemRepository(session)
        count = repo.count_by_library("lib-count")
        assert count == 7

        # Non-existent library
        count = repo.count_by_library("nonexistent")
        assert count == 0


def test_upsert_item(db_path: Path) -> None:
    """Test upserting an item (insert or update)."""
    from submate.database import ItemRepository, LibraryRepository, get_db_session

    with get_db_session(db_path) as session:
        lib_repo = LibraryRepository(session)
        lib_repo.create(id="lib-upsert", name="Movies", type="movies", target_languages=["en"])

    # First upsert - should create
    with get_db_session(db_path) as session:
        repo = ItemRepository(session)
        item = repo.upsert(
            id="ups-item",
            library_id="lib-upsert",
            type="movie",
            title="Original Title",
            path="/media/movies/upsert.mkv",
        )
        assert item.title == "Original Title"

    # Second upsert - should update
    with get_db_session(db_path) as session:
        repo = ItemRepository(session)
        item = repo.upsert(
            id="ups-item",
            library_id="lib-upsert",
            type="movie",
            title="Updated Title",
            path="/media/movies/upsert.mkv",
        )
        assert item.title == "Updated Title"

    # Verify only one item exists
    with get_db_session(db_path) as session:
        repo = ItemRepository(session)
        count = repo.count_by_library("lib-upsert")
        assert count == 1


def test_create_subtitle(db_path: Path) -> None:
    """Test creating a subtitle through the repository."""
    from submate.database import (
        ItemRepository,
        LibraryRepository,
        SubtitleRepository,
        get_db_session,
    )

    # Create library and item first
    with get_db_session(db_path) as session:
        lib_repo = LibraryRepository(session)
        lib_repo.create(id="lib-sub", name="Movies", type="movies", target_languages=["en"])

        item_repo = ItemRepository(session)
        item_repo.create(
            id="item-sub",
            library_id="lib-sub",
            type="movie",
            title="Test Movie",
            path="/media/movies/sub.mkv",
        )

    # Create subtitle
    with get_db_session(db_path) as session:
        repo = SubtitleRepository(session)
        subtitle = repo.create(
            item_id="item-sub",
            language="en",
            source="generated",
            path="/media/movies/sub.en.srt",
        )

        assert subtitle.id is not None
        assert subtitle.item_id == "item-sub"
        assert subtitle.language == "en"
        assert subtitle.source == "generated"
        assert subtitle.path == "/media/movies/sub.en.srt"
        assert subtitle.created_at is not None


def test_get_subtitle_by_item_and_language(db_path: Path) -> None:
    """Test getting a subtitle by item ID and language."""
    from submate.database import (
        ItemRepository,
        LibraryRepository,
        SubtitleRepository,
        get_db_session,
    )

    with get_db_session(db_path) as session:
        lib_repo = LibraryRepository(session)
        lib_repo.create(id="lib-subget", name="Movies", type="movies", target_languages=["en"])

        item_repo = ItemRepository(session)
        item_repo.create(
            id="item-subget",
            library_id="lib-subget",
            type="movie",
            title="Test Movie",
            path="/media/movies/subget.mkv",
        )

        sub_repo = SubtitleRepository(session)
        sub_repo.create(
            item_id="item-subget",
            language="en",
            source="external",
            path="/media/movies/subget.en.srt",
        )
        sub_repo.create(
            item_id="item-subget",
            language="es",
            source="generated",
            path="/media/movies/subget.es.srt",
        )

    with get_db_session(db_path) as session:
        repo = SubtitleRepository(session)

        # Find English subtitle
        sub = repo.get_by_item_and_language("item-subget", "en")
        assert sub is not None
        assert sub.language == "en"
        assert sub.source == "external"

        # Find Spanish subtitle
        sub = repo.get_by_item_and_language("item-subget", "es")
        assert sub is not None
        assert sub.language == "es"
        assert sub.source == "generated"

        # Non-existent
        sub = repo.get_by_item_and_language("item-subget", "fr")
        assert sub is None


def test_list_subtitles_by_item(db_path: Path) -> None:
    """Test listing all subtitles for an item."""
    from submate.database import (
        ItemRepository,
        LibraryRepository,
        SubtitleRepository,
        get_db_session,
    )

    with get_db_session(db_path) as session:
        lib_repo = LibraryRepository(session)
        lib_repo.create(id="lib-sublist", name="Movies", type="movies", target_languages=["en"])

        item_repo = ItemRepository(session)
        item_repo.create(
            id="item-sublist",
            library_id="lib-sublist",
            type="movie",
            title="Test Movie",
            path="/media/movies/sublist.mkv",
        )

        sub_repo = SubtitleRepository(session)
        for lang in ["en", "es", "fr"]:
            sub_repo.create(
                item_id="item-sublist",
                language=lang,
                source="generated",
                path=f"/media/movies/sublist.{lang}.srt",
            )

    with get_db_session(db_path) as session:
        repo = SubtitleRepository(session)
        subs = repo.list_by_item("item-sublist")
        assert len(subs) == 3
        languages = {s.language for s in subs}
        assert languages == {"en", "es", "fr"}


def test_delete_subtitle(db_path: Path) -> None:
    """Test deleting a subtitle."""
    from submate.database import (
        ItemRepository,
        LibraryRepository,
        SubtitleRepository,
        get_db_session,
    )

    with get_db_session(db_path) as session:
        lib_repo = LibraryRepository(session)
        lib_repo.create(id="lib-subdel", name="Movies", type="movies", target_languages=["en"])

        item_repo = ItemRepository(session)
        item_repo.create(
            id="item-subdel",
            library_id="lib-subdel",
            type="movie",
            title="Test Movie",
            path="/media/movies/subdel.mkv",
        )

        sub_repo = SubtitleRepository(session)
        subtitle = sub_repo.create(
            item_id="item-subdel",
            language="en",
            source="generated",
            path="/media/movies/subdel.en.srt",
        )
        sub_id = subtitle.id

    with get_db_session(db_path) as session:
        repo = SubtitleRepository(session)
        result = repo.delete(sub_id)
        assert result is True

        # Should not exist anymore
        subs = repo.list_by_item("item-subdel")
        assert len(subs) == 0

        # Deleting non-existent returns False
        result = repo.delete(99999)
        assert result is False


def test_upsert_subtitle(db_path: Path) -> None:
    """Test upserting a subtitle."""
    from submate.database import (
        ItemRepository,
        LibraryRepository,
        SubtitleRepository,
        get_db_session,
    )

    with get_db_session(db_path) as session:
        lib_repo = LibraryRepository(session)
        lib_repo.create(id="lib-subups", name="Movies", type="movies", target_languages=["en"])

        item_repo = ItemRepository(session)
        item_repo.create(
            id="item-subups",
            library_id="lib-subups",
            type="movie",
            title="Test Movie",
            path="/media/movies/subups.mkv",
        )

    # First upsert - should create
    with get_db_session(db_path) as session:
        repo = SubtitleRepository(session)
        sub = repo.upsert(
            item_id="item-subups",
            language="en",
            source="external",
            path="/media/movies/subups.en.srt",
        )
        assert sub.source == "external"

    # Second upsert - should update
    with get_db_session(db_path) as session:
        repo = SubtitleRepository(session)
        sub = repo.upsert(
            item_id="item-subups",
            language="en",
            source="generated",
            path="/media/movies/subups.en.new.srt",
        )
        assert sub.source == "generated"
        assert sub.path == "/media/movies/subups.en.new.srt"

    # Verify only one subtitle exists
    with get_db_session(db_path) as session:
        repo = SubtitleRepository(session)
        subs = repo.list_by_item("item-subups")
        assert len(subs) == 1


def test_create_job(db_path: Path) -> None:
    """Test creating a job through the repository."""
    from submate.database import (
        ItemRepository,
        JobRepository,
        LibraryRepository,
        get_db_session,
    )

    # Create library and item first
    with get_db_session(db_path) as session:
        lib_repo = LibraryRepository(session)
        lib_repo.create(id="lib-job", name="Movies", type="movies", target_languages=["en"])

        item_repo = ItemRepository(session)
        item_repo.create(
            id="item-job",
            library_id="lib-job",
            type="movie",
            title="Test Movie",
            path="/media/movies/job.mkv",
        )

    # Create job
    with get_db_session(db_path) as session:
        repo = JobRepository(session)
        job = repo.create(
            id="job-123",
            item_id="item-job",
            language="en",
            status="pending",
        )

        assert job.id == "job-123"
        assert job.item_id == "item-job"
        assert job.language == "en"
        assert job.status == "pending"
        assert job.created_at is not None
        assert job.started_at is None
        assert job.completed_at is None


def test_get_job_by_id(db_path: Path) -> None:
    """Test getting a job by ID."""
    from submate.database import (
        ItemRepository,
        JobRepository,
        LibraryRepository,
        get_db_session,
    )

    with get_db_session(db_path) as session:
        lib_repo = LibraryRepository(session)
        lib_repo.create(id="lib-jobget", name="Movies", type="movies", target_languages=["en"])

        item_repo = ItemRepository(session)
        item_repo.create(
            id="item-jobget",
            library_id="lib-jobget",
            type="movie",
            title="Test Movie",
            path="/media/movies/jobget.mkv",
        )

        job_repo = JobRepository(session)
        job_repo.create(id="job-get", item_id="item-jobget", language="en", status="pending")

    with get_db_session(db_path) as session:
        repo = JobRepository(session)
        job = repo.get_by_id("job-get")
        assert job is not None
        assert job.id == "job-get"

        # Non-existent
        job = repo.get_by_id("nonexistent")
        assert job is None


def test_list_jobs_by_status(db_path: Path) -> None:
    """Test listing jobs by status with pagination."""
    from submate.database import (
        ItemRepository,
        JobRepository,
        LibraryRepository,
        get_db_session,
    )

    with get_db_session(db_path) as session:
        lib_repo = LibraryRepository(session)
        lib_repo.create(id="lib-jobstat", name="Movies", type="movies", target_languages=["en"])

        item_repo = ItemRepository(session)
        item_repo.create(
            id="item-jobstat",
            library_id="lib-jobstat",
            type="movie",
            title="Test Movie",
            path="/media/movies/jobstat.mkv",
        )

        job_repo = JobRepository(session)
        # Create pending jobs
        for i in range(3):
            job_repo.create(
                id=f"job-pend-{i}",
                item_id="item-jobstat",
                language="en",
                status="pending",
            )
        # Create completed jobs
        for i in range(2):
            job_repo.create(
                id=f"job-comp-{i}",
                item_id="item-jobstat",
                language="en",
                status="completed",
            )

    with get_db_session(db_path) as session:
        repo = JobRepository(session)

        pending = repo.list_by_status("pending")
        assert len(pending) == 3

        completed = repo.list_by_status("completed")
        assert len(completed) == 2

        # Test pagination
        pending_page = repo.list_by_status("pending", limit=2, offset=0)
        assert len(pending_page) == 2


def test_list_jobs_by_item(db_path: Path) -> None:
    """Test listing jobs by item ID."""
    from submate.database import (
        ItemRepository,
        JobRepository,
        LibraryRepository,
        get_db_session,
    )

    with get_db_session(db_path) as session:
        lib_repo = LibraryRepository(session)
        lib_repo.create(id="lib-jobitem", name="Movies", type="movies", target_languages=["en"])

        item_repo = ItemRepository(session)
        item_repo.create(
            id="item-jobitem1",
            library_id="lib-jobitem",
            type="movie",
            title="Movie 1",
            path="/media/movies/jobitem1.mkv",
        )
        item_repo.create(
            id="item-jobitem2",
            library_id="lib-jobitem",
            type="movie",
            title="Movie 2",
            path="/media/movies/jobitem2.mkv",
        )

        job_repo = JobRepository(session)
        job_repo.create(id="job-i1-en", item_id="item-jobitem1", language="en", status="pending")
        job_repo.create(id="job-i1-es", item_id="item-jobitem1", language="es", status="pending")
        job_repo.create(id="job-i2-en", item_id="item-jobitem2", language="en", status="pending")

    with get_db_session(db_path) as session:
        repo = JobRepository(session)
        jobs_item1 = repo.list_by_item("item-jobitem1")
        assert len(jobs_item1) == 2

        jobs_item2 = repo.list_by_item("item-jobitem2")
        assert len(jobs_item2) == 1


def test_update_job_status(db_path: Path) -> None:
    """Test updating job status with timestamp handling."""
    from submate.database import (
        ItemRepository,
        JobRepository,
        LibraryRepository,
        get_db_session,
    )

    with get_db_session(db_path) as session:
        lib_repo = LibraryRepository(session)
        lib_repo.create(id="lib-jobupd", name="Movies", type="movies", target_languages=["en"])

        item_repo = ItemRepository(session)
        item_repo.create(
            id="item-jobupd",
            library_id="lib-jobupd",
            type="movie",
            title="Test Movie",
            path="/media/movies/jobupd.mkv",
        )

        job_repo = JobRepository(session)
        job_repo.create(id="job-upd", item_id="item-jobupd", language="en", status="pending")

    # Update to running
    with get_db_session(db_path) as session:
        repo = JobRepository(session)
        job = repo.update_status("job-upd", "running")
        assert job is not None
        assert job.status == "running"
        assert job.started_at is not None
        assert job.completed_at is None

    # Update to completed
    with get_db_session(db_path) as session:
        repo = JobRepository(session)
        job = repo.update_status("job-upd", "completed")
        assert job is not None
        assert job.status == "completed"
        assert job.completed_at is not None

    # Test updating with error for failed status
    with get_db_session(db_path) as session:
        job_repo = JobRepository(session)
        job_repo.create(id="job-fail", item_id="item-jobupd", language="es", status="pending")

    with get_db_session(db_path) as session:
        repo = JobRepository(session)
        job = repo.update_status("job-fail", "failed", error="Transcription failed")
        assert job is not None
        assert job.status == "failed"
        assert job.error == "Transcription failed"
        assert job.completed_at is not None

    # Non-existent job
    with get_db_session(db_path) as session:
        repo = JobRepository(session)
        job = repo.update_status("nonexistent", "running")
        assert job is None


def test_count_jobs_by_status(db_path: Path) -> None:
    """Test counting jobs by status."""
    from submate.database import (
        ItemRepository,
        JobRepository,
        LibraryRepository,
        get_db_session,
    )

    with get_db_session(db_path) as session:
        lib_repo = LibraryRepository(session)
        lib_repo.create(id="lib-jobcnt", name="Movies", type="movies", target_languages=["en"])

        item_repo = ItemRepository(session)
        item_repo.create(
            id="item-jobcnt",
            library_id="lib-jobcnt",
            type="movie",
            title="Test Movie",
            path="/media/movies/jobcnt.mkv",
        )

        job_repo = JobRepository(session)
        for i in range(5):
            job_repo.create(
                id=f"job-cnt-pend-{i}",
                item_id="item-jobcnt",
                language="en",
                status="pending",
            )
        for i in range(3):
            job_repo.create(
                id=f"job-cnt-run-{i}",
                item_id="item-jobcnt",
                language="en",
                status="running",
            )

    with get_db_session(db_path) as session:
        repo = JobRepository(session)
        assert repo.count_by_status("pending") == 5
        assert repo.count_by_status("running") == 3
        assert repo.count_by_status("completed") == 0
