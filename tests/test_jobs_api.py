"""Tests for Jobs API endpoints."""

from pathlib import Path

import pytest
from fastapi.testclient import TestClient

from submate.database.repository import ItemRepository, JobRepository, LibraryRepository, SubtitleRepository
from submate.database.session import get_db_session, init_database
from submate.server import app


@pytest.fixture
def client():
    """FastAPI test client."""
    return TestClient(app)


@pytest.fixture
def db_path(tmp_path: Path) -> Path:
    """Create a temporary database for testing."""
    db_file = tmp_path / "test.db"
    init_database(db_file)
    return db_file


def test_transcribe_item(client: TestClient, db_path: Path, mocker):
    """Test POST /api/items/{item_id}/transcribe creates a job."""
    mocker.patch(
        "submate.server.handlers.jobs.router._get_db_path",
        return_value=db_path,
    )
    # Mock event bus to verify job.created event is published
    mock_event_bus = mocker.MagicMock()
    mocker.patch(
        "submate.server.handlers.jobs.router.get_event_bus",
        return_value=mock_event_bus,
    )

    # Create test library and item
    with get_db_session(db_path) as session:
        library_repo = LibraryRepository(session)
        item_repo = ItemRepository(session)

        library_repo.create(
            id="lib-1",
            name="Movies",
            type="movies",
            target_languages=["en", "es"],
            skip_existing=True,
            enabled=True,
        )
        item_repo.create(
            id="movie-1",
            library_id="lib-1",
            type="movie",
            title="Test Movie",
            path="/media/movies/test.mkv",
        )

    response = client.post(
        "/api/items/movie-1/transcribe",
        json={"language": "en"},
    )

    assert response.status_code == 200
    data = response.json()
    assert "job_id" in data
    assert data["message"] == "Transcription job queued"

    # Verify job was created in database
    with get_db_session(db_path) as session:
        job_repo = JobRepository(session)
        job = job_repo.get_by_id(data["job_id"])
        assert job is not None
        assert job.item_id == "movie-1"
        assert job.language == "en"
        assert job.status == "pending"

    # Verify event was published
    mock_event_bus.publish.assert_called_once()
    call_args = mock_event_bus.publish.call_args
    assert call_args[0][0] == "job.created"
    assert call_args[0][1]["job_id"] == data["job_id"]


def test_transcribe_item_not_found(client: TestClient, db_path: Path, mocker):
    """Test POST /api/items/{item_id}/transcribe returns 404 for non-existent item."""
    mocker.patch(
        "submate.server.handlers.jobs.router._get_db_path",
        return_value=db_path,
    )

    response = client.post(
        "/api/items/non-existent/transcribe",
        json={"language": "en"},
    )

    assert response.status_code == 404
    assert response.json()["detail"] == "Item not found"


def test_transcribe_library(client: TestClient, db_path: Path, mocker):
    """Test POST /api/libraries/{library_id}/transcribe queues jobs for items missing subtitles."""
    mocker.patch(
        "submate.server.handlers.jobs.router._get_db_path",
        return_value=db_path,
    )
    mock_event_bus = mocker.MagicMock()
    mocker.patch(
        "submate.server.handlers.jobs.router.get_event_bus",
        return_value=mock_event_bus,
    )

    # Create test library with items - one has subtitle, two don't
    with get_db_session(db_path) as session:
        library_repo = LibraryRepository(session)
        item_repo = ItemRepository(session)
        subtitle_repo = SubtitleRepository(session)

        library_repo.create(
            id="lib-1",
            name="Movies",
            type="movies",
            target_languages=["en", "es"],
            skip_existing=True,
            enabled=True,
        )

        # Movie with existing English subtitle
        item_repo.create(
            id="movie-1",
            library_id="lib-1",
            type="movie",
            title="Test Movie 1",
            path="/media/movies/test1.mkv",
        )
        subtitle_repo.create(
            item_id="movie-1",
            language="en",
            source="external",
            path="/media/movies/test1.en.srt",
        )

        # Movie without English subtitle
        item_repo.create(
            id="movie-2",
            library_id="lib-1",
            type="movie",
            title="Test Movie 2",
            path="/media/movies/test2.mkv",
        )

        # Another movie without English subtitle
        item_repo.create(
            id="movie-3",
            library_id="lib-1",
            type="movie",
            title="Test Movie 3",
            path="/media/movies/test3.mkv",
        )

    response = client.post(
        "/api/libraries/lib-1/transcribe",
        json={"language": "en"},
    )

    assert response.status_code == 200
    data = response.json()
    assert data["total_queued"] == 2  # Only movies without English subtitles
    assert len(data["jobs"]) == 2

    # Verify jobs were created for items without English subtitles
    with get_db_session(db_path) as session:
        job_repo = JobRepository(session)

        # Should have jobs for movie-2 and movie-3
        job_item_ids = set()
        for job_response in data["jobs"]:
            job = job_repo.get_by_id(job_response["job_id"])
            assert job is not None
            assert job.status == "pending"
            assert job.language == "en"
            job_item_ids.add(job.item_id)

        assert job_item_ids == {"movie-2", "movie-3"}


def test_transcribe_library_not_found(client: TestClient, db_path: Path, mocker):
    """Test POST /api/libraries/{library_id}/transcribe returns 404 for non-existent library."""
    mocker.patch(
        "submate.server.handlers.jobs.router._get_db_path",
        return_value=db_path,
    )

    response = client.post(
        "/api/libraries/non-existent/transcribe",
        json={"language": "en"},
    )

    assert response.status_code == 404
    assert response.json()["detail"] == "Library not found"


def test_bulk_transcribe(client: TestClient, db_path: Path, mocker):
    """Test POST /api/bulk/transcribe queues jobs for selected items."""
    mocker.patch(
        "submate.server.handlers.jobs.router._get_db_path",
        return_value=db_path,
    )
    mock_event_bus = mocker.MagicMock()
    mocker.patch(
        "submate.server.handlers.jobs.router.get_event_bus",
        return_value=mock_event_bus,
    )

    # Create test library and items
    with get_db_session(db_path) as session:
        library_repo = LibraryRepository(session)
        item_repo = ItemRepository(session)

        library_repo.create(
            id="lib-1",
            name="Movies",
            type="movies",
            target_languages=["en"],
            skip_existing=True,
            enabled=True,
        )

        for i in range(3):
            item_repo.create(
                id=f"movie-{i}",
                library_id="lib-1",
                type="movie",
                title=f"Test Movie {i}",
                path=f"/media/movies/test{i}.mkv",
            )

    response = client.post(
        "/api/bulk/transcribe",
        json={
            "item_ids": ["movie-0", "movie-1"],
            "language": "es",
        },
    )

    assert response.status_code == 200
    data = response.json()
    assert data["total_queued"] == 2
    assert len(data["jobs"]) == 2

    # Verify jobs were created
    with get_db_session(db_path) as session:
        job_repo = JobRepository(session)
        for job_response in data["jobs"]:
            job = job_repo.get_by_id(job_response["job_id"])
            assert job is not None
            assert job.language == "es"
            assert job.status == "pending"


def test_list_jobs(client: TestClient, db_path: Path, mocker):
    """Test GET /api/jobs lists jobs with pagination."""
    mocker.patch(
        "submate.server.handlers.jobs.router._get_db_path",
        return_value=db_path,
    )

    # Create test library, items, and jobs
    with get_db_session(db_path) as session:
        library_repo = LibraryRepository(session)
        item_repo = ItemRepository(session)
        job_repo = JobRepository(session)

        library_repo.create(
            id="lib-1",
            name="Movies",
            type="movies",
            target_languages=["en"],
            skip_existing=True,
            enabled=True,
        )

        # Create items and jobs
        for i in range(5):
            item_repo.create(
                id=f"movie-{i}",
                library_id="lib-1",
                type="movie",
                title=f"Test Movie {i}",
                path=f"/media/movies/test{i}.mkv",
            )
            status = "pending" if i < 3 else "completed"
            job_repo.create(
                id=f"job-{i}",
                item_id=f"movie-{i}",
                language="en",
                status=status,
            )

    # Test listing all jobs
    response = client.get("/api/jobs")
    assert response.status_code == 200
    data = response.json()
    assert data["total"] == 5
    assert data["page"] == 1
    assert data["page_size"] == 50
    assert len(data["jobs"]) == 5

    # Test filtering by status
    response = client.get("/api/jobs?status=pending")
    assert response.status_code == 200
    data = response.json()
    assert data["total"] == 3
    assert all(job["status"] == "pending" for job in data["jobs"])

    # Test pagination
    response = client.get("/api/jobs?page=1&page_size=2")
    assert response.status_code == 200
    data = response.json()
    assert data["total"] == 5
    assert data["page"] == 1
    assert data["page_size"] == 2
    assert len(data["jobs"]) == 2


def test_list_jobs_includes_item_title(client: TestClient, db_path: Path, mocker):
    """Test GET /api/jobs includes item_title from joined Item."""
    mocker.patch(
        "submate.server.handlers.jobs.router._get_db_path",
        return_value=db_path,
    )

    with get_db_session(db_path) as session:
        library_repo = LibraryRepository(session)
        item_repo = ItemRepository(session)
        job_repo = JobRepository(session)

        library_repo.create(
            id="lib-1",
            name="Movies",
            type="movies",
            target_languages=["en"],
            skip_existing=True,
            enabled=True,
        )
        item_repo.create(
            id="movie-1",
            library_id="lib-1",
            type="movie",
            title="The Great Movie",
            path="/media/movies/test.mkv",
        )
        job_repo.create(
            id="job-1",
            item_id="movie-1",
            language="en",
            status="pending",
        )

    response = client.get("/api/jobs")
    assert response.status_code == 200
    data = response.json()
    assert len(data["jobs"]) == 1
    assert data["jobs"][0]["item_title"] == "The Great Movie"


def test_retry_failed_job(client: TestClient, db_path: Path, mocker):
    """Test POST /api/jobs/{job_id}/retry resets failed job to pending."""
    mocker.patch(
        "submate.server.handlers.jobs.router._get_db_path",
        return_value=db_path,
    )
    mock_event_bus = mocker.MagicMock()
    mocker.patch(
        "submate.server.handlers.jobs.router.get_event_bus",
        return_value=mock_event_bus,
    )

    # Create test library, item, and failed job
    with get_db_session(db_path) as session:
        library_repo = LibraryRepository(session)
        item_repo = ItemRepository(session)
        job_repo = JobRepository(session)

        library_repo.create(
            id="lib-1",
            name="Movies",
            type="movies",
            target_languages=["en"],
            skip_existing=True,
            enabled=True,
        )
        item_repo.create(
            id="movie-1",
            library_id="lib-1",
            type="movie",
            title="Test Movie",
            path="/media/movies/test.mkv",
        )
        job_repo.create(
            id="job-1",
            item_id="movie-1",
            language="en",
            status="failed",
        )
        # Update to add error message
        job_repo.update_status("job-1", "failed", error="Transcription failed")

    response = client.post("/api/jobs/job-1/retry")

    assert response.status_code == 200
    data = response.json()
    assert data["id"] == "job-1"
    assert data["status"] == "pending"
    assert data["error"] is None

    # Verify job was updated in database
    with get_db_session(db_path) as session:
        job_repo = JobRepository(session)
        job = job_repo.get_by_id("job-1")
        assert job is not None
        assert job.status == "pending"
        assert job.error is None


def test_retry_non_failed_job_returns_400(client: TestClient, db_path: Path, mocker):
    """Test POST /api/jobs/{job_id}/retry returns 400 for non-failed job."""
    mocker.patch(
        "submate.server.handlers.jobs.router._get_db_path",
        return_value=db_path,
    )

    # Create a pending job
    with get_db_session(db_path) as session:
        library_repo = LibraryRepository(session)
        item_repo = ItemRepository(session)
        job_repo = JobRepository(session)

        library_repo.create(
            id="lib-1",
            name="Movies",
            type="movies",
            target_languages=["en"],
            skip_existing=True,
            enabled=True,
        )
        item_repo.create(
            id="movie-1",
            library_id="lib-1",
            type="movie",
            title="Test Movie",
            path="/media/movies/test.mkv",
        )
        job_repo.create(
            id="job-1",
            item_id="movie-1",
            language="en",
            status="pending",
        )

    response = client.post("/api/jobs/job-1/retry")

    assert response.status_code == 400
    assert response.json()["detail"] == "Only failed jobs can be retried"


def test_retry_job_not_found(client: TestClient, db_path: Path, mocker):
    """Test POST /api/jobs/{job_id}/retry returns 404 for non-existent job."""
    mocker.patch(
        "submate.server.handlers.jobs.router._get_db_path",
        return_value=db_path,
    )

    response = client.post("/api/jobs/non-existent/retry")

    assert response.status_code == 404
    assert response.json()["detail"] == "Job not found"


def test_cancel_pending_job(client: TestClient, db_path: Path, mocker):
    """Test DELETE /api/jobs/{job_id} cancels pending job."""
    mocker.patch(
        "submate.server.handlers.jobs.router._get_db_path",
        return_value=db_path,
    )

    # Create a pending job
    with get_db_session(db_path) as session:
        library_repo = LibraryRepository(session)
        item_repo = ItemRepository(session)
        job_repo = JobRepository(session)

        library_repo.create(
            id="lib-1",
            name="Movies",
            type="movies",
            target_languages=["en"],
            skip_existing=True,
            enabled=True,
        )
        item_repo.create(
            id="movie-1",
            library_id="lib-1",
            type="movie",
            title="Test Movie",
            path="/media/movies/test.mkv",
        )
        job_repo.create(
            id="job-1",
            item_id="movie-1",
            language="en",
            status="pending",
        )

    response = client.delete("/api/jobs/job-1")

    assert response.status_code == 204

    # Verify job was deleted
    with get_db_session(db_path) as session:
        job_repo = JobRepository(session)
        job = job_repo.get_by_id("job-1")
        assert job is None


def test_cancel_non_pending_job_returns_400(client: TestClient, db_path: Path, mocker):
    """Test DELETE /api/jobs/{job_id} returns 400 for non-pending job."""
    mocker.patch(
        "submate.server.handlers.jobs.router._get_db_path",
        return_value=db_path,
    )

    # Create a running job
    with get_db_session(db_path) as session:
        library_repo = LibraryRepository(session)
        item_repo = ItemRepository(session)
        job_repo = JobRepository(session)

        library_repo.create(
            id="lib-1",
            name="Movies",
            type="movies",
            target_languages=["en"],
            skip_existing=True,
            enabled=True,
        )
        item_repo.create(
            id="movie-1",
            library_id="lib-1",
            type="movie",
            title="Test Movie",
            path="/media/movies/test.mkv",
        )
        job_repo.create(
            id="job-1",
            item_id="movie-1",
            language="en",
            status="running",
        )

    response = client.delete("/api/jobs/job-1")

    assert response.status_code == 400
    assert response.json()["detail"] == "Only pending jobs can be cancelled"


def test_cancel_job_not_found(client: TestClient, db_path: Path, mocker):
    """Test DELETE /api/jobs/{job_id} returns 404 for non-existent job."""
    mocker.patch(
        "submate.server.handlers.jobs.router._get_db_path",
        return_value=db_path,
    )

    response = client.delete("/api/jobs/non-existent")

    assert response.status_code == 404
    assert response.json()["detail"] == "Job not found"
