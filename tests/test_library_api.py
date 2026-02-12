"""Tests for Library API endpoints."""

from pathlib import Path

import pytest
from fastapi.testclient import TestClient

from submate.database.repository import ItemRepository, LibraryRepository
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


def test_get_libraries_empty(client: TestClient, db_path: Path, mocker):
    """Test GET /api/libraries returns empty list when no libraries exist."""
    # Mock the database path helper to use our test database
    mocker.patch(
        "submate.server.handlers.library.router._get_db_path",
        return_value=db_path,
    )

    response = client.get("/api/libraries")

    assert response.status_code == 200
    data = response.json()
    assert data["libraries"] == []
    assert data["total"] == 0


def test_get_libraries_with_data(client: TestClient, db_path: Path, mocker):
    """Test GET /api/libraries returns libraries with item counts."""
    # Mock the database path helper to use our test database
    mocker.patch(
        "submate.server.handlers.library.router._get_db_path",
        return_value=db_path,
    )

    # Create test libraries and items
    with get_db_session(db_path) as session:
        library_repo = LibraryRepository(session)
        item_repo = ItemRepository(session)

        # Create a movies library with 2 items
        library_repo.create(
            id="lib-1",
            name="Movies",
            type="movies",
            target_languages=["en", "es"],
            skip_existing=True,
            enabled=True,
        )
        item_repo.create(
            id="item-1",
            library_id="lib-1",
            type="movie",
            title="Test Movie 1",
            path="/media/movies/test1.mkv",
        )
        item_repo.create(
            id="item-2",
            library_id="lib-1",
            type="movie",
            title="Test Movie 2",
            path="/media/movies/test2.mkv",
        )

        # Create a series library with 1 item
        library_repo.create(
            id="lib-2",
            name="TV Shows",
            type="series",
            target_languages=["en"],
            skip_existing=False,
            enabled=False,
        )
        item_repo.create(
            id="item-3",
            library_id="lib-2",
            type="episode",
            title="Test Episode",
            path="/media/series/test.mkv",
            series_id="series-1",
            series_name="Test Series",
            season_num=1,
            episode_num=1,
        )

    response = client.get("/api/libraries")

    assert response.status_code == 200
    data = response.json()
    assert data["total"] == 2
    assert len(data["libraries"]) == 2

    # Find libraries by ID
    movies_lib = next((lib for lib in data["libraries"] if lib["id"] == "lib-1"), None)
    series_lib = next((lib for lib in data["libraries"] if lib["id"] == "lib-2"), None)

    assert movies_lib is not None
    assert movies_lib["name"] == "Movies"
    assert movies_lib["type"] == "movies"
    assert movies_lib["target_languages"] == ["en", "es"]
    assert movies_lib["skip_existing"] is True
    assert movies_lib["enabled"] is True
    assert movies_lib["item_count"] == 2

    assert series_lib is not None
    assert series_lib["name"] == "TV Shows"
    assert series_lib["type"] == "series"
    assert series_lib["target_languages"] == ["en"]
    assert series_lib["skip_existing"] is False
    assert series_lib["enabled"] is False
    assert series_lib["item_count"] == 1


def test_get_library_by_id(client: TestClient, db_path: Path, mocker):
    """Test GET /api/libraries/{library_id} returns a single library."""
    mocker.patch(
        "submate.server.handlers.library.router._get_db_path",
        return_value=db_path,
    )

    # Create test library
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
        item_repo.create(
            id="item-1",
            library_id="lib-1",
            type="movie",
            title="Test Movie",
            path="/media/movies/test.mkv",
        )

    response = client.get("/api/libraries/lib-1")

    assert response.status_code == 200
    data = response.json()
    assert data["id"] == "lib-1"
    assert data["name"] == "Movies"
    assert data["type"] == "movies"
    assert data["target_languages"] == ["en"]
    assert data["skip_existing"] is True
    assert data["enabled"] is True
    assert data["item_count"] == 1


def test_get_library_not_found(client: TestClient, db_path: Path, mocker):
    """Test GET /api/libraries/{library_id} returns 404 for non-existent library."""
    mocker.patch(
        "submate.server.handlers.library.router._get_db_path",
        return_value=db_path,
    )

    response = client.get("/api/libraries/non-existent")

    assert response.status_code == 404
    assert response.json()["detail"] == "Library not found"


def test_update_library(client: TestClient, db_path: Path, mocker):
    """Test PATCH /api/libraries/{library_id} updates library settings."""
    mocker.patch(
        "submate.server.handlers.library.router._get_db_path",
        return_value=db_path,
    )

    # Create test library
    with get_db_session(db_path) as session:
        library_repo = LibraryRepository(session)
        library_repo.create(
            id="lib-1",
            name="Movies",
            type="movies",
            target_languages=["en"],
            skip_existing=True,
            enabled=True,
        )

    # Update library settings
    response = client.patch(
        "/api/libraries/lib-1",
        json={
            "target_languages": ["en", "es", "fr"],
            "skip_existing": False,
            "enabled": False,
        },
    )

    assert response.status_code == 200
    data = response.json()
    assert data["id"] == "lib-1"
    assert data["target_languages"] == ["en", "es", "fr"]
    assert data["skip_existing"] is False
    assert data["enabled"] is False


def test_update_library_partial(client: TestClient, db_path: Path, mocker):
    """Test PATCH /api/libraries/{library_id} with partial update."""
    mocker.patch(
        "submate.server.handlers.library.router._get_db_path",
        return_value=db_path,
    )

    # Create test library
    with get_db_session(db_path) as session:
        library_repo = LibraryRepository(session)
        library_repo.create(
            id="lib-1",
            name="Movies",
            type="movies",
            target_languages=["en"],
            skip_existing=True,
            enabled=True,
        )

    # Update only enabled field
    response = client.patch(
        "/api/libraries/lib-1",
        json={"enabled": False},
    )

    assert response.status_code == 200
    data = response.json()
    assert data["id"] == "lib-1"
    # Original values should be preserved
    assert data["target_languages"] == ["en"]
    assert data["skip_existing"] is True
    # Updated value
    assert data["enabled"] is False


def test_update_library_not_found(client: TestClient, db_path: Path, mocker):
    """Test PATCH /api/libraries/{library_id} returns 404 for non-existent library."""
    mocker.patch(
        "submate.server.handlers.library.router._get_db_path",
        return_value=db_path,
    )

    response = client.patch(
        "/api/libraries/non-existent",
        json={"enabled": False},
    )

    assert response.status_code == 404
    assert response.json()["detail"] == "Library not found"
