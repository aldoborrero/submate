"""Tests for Items API endpoints."""

from pathlib import Path

import pytest
from fastapi.testclient import TestClient

from submate.database.repository import ItemRepository, LibraryRepository, SubtitleRepository
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


def test_get_movies_empty(client: TestClient, db_path: Path, mocker):
    """Test GET /api/movies returns empty list when no movies exist."""
    mocker.patch(
        "submate.server.handlers.items.router._get_db_path",
        return_value=db_path,
    )

    response = client.get("/api/movies")

    assert response.status_code == 200
    data = response.json()
    assert data["items"] == []
    assert data["total"] == 0
    assert data["page"] == 1
    assert data["page_size"] == 50


def test_get_movies_with_data(client: TestClient, db_path: Path, mocker):
    """Test GET /api/movies returns movies with pagination."""
    mocker.patch(
        "submate.server.handlers.items.router._get_db_path",
        return_value=db_path,
    )

    # Create test library and movies
    with get_db_session(db_path) as session:
        library_repo = LibraryRepository(session)
        item_repo = ItemRepository(session)
        subtitle_repo = SubtitleRepository(session)

        # Create a movies library
        library_repo.create(
            id="lib-1",
            name="Movies",
            type="movies",
            target_languages=["en", "es"],
            skip_existing=True,
            enabled=True,
        )

        # Create movies
        item_repo.create(
            id="movie-1",
            library_id="lib-1",
            type="movie",
            title="Test Movie 1",
            path="/media/movies/test1.mkv",
            poster_url="/Items/movie-1/Images/Primary",
        )
        item_repo.create(
            id="movie-2",
            library_id="lib-1",
            type="movie",
            title="Test Movie 2",
            path="/media/movies/test2.mkv",
        )

        # Add subtitle for first movie
        subtitle_repo.create(
            item_id="movie-1",
            language="en",
            source="external",
            path="/media/movies/test1.en.srt",
        )

    response = client.get("/api/movies")

    assert response.status_code == 200
    data = response.json()
    assert data["total"] == 2
    assert data["page"] == 1
    assert data["page_size"] == 50
    assert len(data["items"]) == 2

    # Find movies by ID
    movie1 = next((item for item in data["items"] if item["id"] == "movie-1"), None)
    movie2 = next((item for item in data["items"] if item["id"] == "movie-2"), None)

    assert movie1 is not None
    assert movie1["title"] == "Test Movie 1"
    assert movie1["type"] == "movie"
    assert movie1["library_id"] == "lib-1"
    assert movie1["path"] == "/media/movies/test1.mkv"
    assert movie1["poster_url"] == "/Items/movie-1/Images/Primary"
    assert movie1["subtitle_languages"] == ["en"]

    assert movie2 is not None
    assert movie2["title"] == "Test Movie 2"
    assert movie2["subtitle_languages"] == []


def test_get_movies_with_library_filter(client: TestClient, db_path: Path, mocker):
    """Test GET /api/movies with library_id filter."""
    mocker.patch(
        "submate.server.handlers.items.router._get_db_path",
        return_value=db_path,
    )

    # Create test libraries and movies
    with get_db_session(db_path) as session:
        library_repo = LibraryRepository(session)
        item_repo = ItemRepository(session)

        # Create two libraries
        library_repo.create(
            id="lib-1",
            name="Movies 1",
            type="movies",
            target_languages=["en"],
            skip_existing=True,
            enabled=True,
        )
        library_repo.create(
            id="lib-2",
            name="Movies 2",
            type="movies",
            target_languages=["es"],
            skip_existing=True,
            enabled=True,
        )

        # Create movies in different libraries
        item_repo.create(
            id="movie-1",
            library_id="lib-1",
            type="movie",
            title="Movie in Lib 1",
            path="/media/movies1/test.mkv",
        )
        item_repo.create(
            id="movie-2",
            library_id="lib-2",
            type="movie",
            title="Movie in Lib 2",
            path="/media/movies2/test.mkv",
        )

    response = client.get("/api/movies?library_id=lib-1")

    assert response.status_code == 200
    data = response.json()
    assert data["total"] == 1
    assert len(data["items"]) == 1
    assert data["items"][0]["id"] == "movie-1"
    assert data["items"][0]["library_id"] == "lib-1"


def test_get_series_with_data(client: TestClient, db_path: Path, mocker):
    """Test GET /api/series returns series."""
    mocker.patch(
        "submate.server.handlers.items.router._get_db_path",
        return_value=db_path,
    )

    # Create test library and series with episodes
    with get_db_session(db_path) as session:
        library_repo = LibraryRepository(session)
        item_repo = ItemRepository(session)

        # Create a series library
        library_repo.create(
            id="lib-1",
            name="TV Shows",
            type="series",
            target_languages=["en"],
            skip_existing=True,
            enabled=True,
        )

        # Create a series entry (type='series')
        item_repo.create(
            id="series-1",
            library_id="lib-1",
            type="series",
            title="Test Series",
            path="/media/series/test",
            poster_url="/Items/series-1/Images/Primary",
        )

        # Create episodes belonging to the series
        item_repo.create(
            id="episode-1",
            library_id="lib-1",
            type="episode",
            title="Episode 1",
            path="/media/series/test/s01e01.mkv",
            series_id="series-1",
            series_name="Test Series",
            season_num=1,
            episode_num=1,
        )
        item_repo.create(
            id="episode-2",
            library_id="lib-1",
            type="episode",
            title="Episode 2",
            path="/media/series/test/s01e02.mkv",
            series_id="series-1",
            series_name="Test Series",
            season_num=1,
            episode_num=2,
        )

    response = client.get("/api/series")

    assert response.status_code == 200
    data = response.json()
    # Should only return series, not episodes
    assert data["total"] == 1
    assert len(data["items"]) == 1

    series = data["items"][0]
    assert series["id"] == "series-1"
    assert series["title"] == "Test Series"
    assert series["type"] == "series"


def test_get_series_detail(client: TestClient, db_path: Path, mocker):
    """Test GET /api/series/{series_id} returns series with episodes."""
    mocker.patch(
        "submate.server.handlers.items.router._get_db_path",
        return_value=db_path,
    )

    # Create test library and series with episodes
    with get_db_session(db_path) as session:
        library_repo = LibraryRepository(session)
        item_repo = ItemRepository(session)
        subtitle_repo = SubtitleRepository(session)

        # Create a series library
        library_repo.create(
            id="lib-1",
            name="TV Shows",
            type="series",
            target_languages=["en"],
            skip_existing=True,
            enabled=True,
        )

        # Create a series entry
        item_repo.create(
            id="series-1",
            library_id="lib-1",
            type="series",
            title="Test Series",
            path="/media/series/test",
        )

        # Create episodes belonging to the series
        item_repo.create(
            id="episode-1",
            library_id="lib-1",
            type="episode",
            title="Episode 1",
            path="/media/series/test/s01e01.mkv",
            series_id="series-1",
            series_name="Test Series",
            season_num=1,
            episode_num=1,
        )
        item_repo.create(
            id="episode-2",
            library_id="lib-1",
            type="episode",
            title="Episode 2",
            path="/media/series/test/s01e02.mkv",
            series_id="series-1",
            series_name="Test Series",
            season_num=1,
            episode_num=2,
        )
        item_repo.create(
            id="episode-3",
            library_id="lib-1",
            type="episode",
            title="Episode 3",
            path="/media/series/test/s02e01.mkv",
            series_id="series-1",
            series_name="Test Series",
            season_num=2,
            episode_num=1,
        )

        # Add subtitle for one episode
        subtitle_repo.create(
            item_id="episode-1",
            language="en",
            source="generated",
            path="/media/series/test/s01e01.en.srt",
        )

    response = client.get("/api/series/series-1")

    assert response.status_code == 200
    data = response.json()

    assert data["id"] == "series-1"
    assert data["title"] == "Test Series"
    assert data["type"] == "series"
    assert data["season_count"] == 2
    assert data["episode_count"] == 3
    assert len(data["episodes"]) == 3

    # Check episode details
    episode1 = next((ep for ep in data["episodes"] if ep["id"] == "episode-1"), None)
    assert episode1 is not None
    assert episode1["season_num"] == 1
    assert episode1["episode_num"] == 1
    assert episode1["subtitle_languages"] == ["en"]


def test_get_series_detail_not_found(client: TestClient, db_path: Path, mocker):
    """Test GET /api/series/{series_id} returns 404 for non-existent series."""
    mocker.patch(
        "submate.server.handlers.items.router._get_db_path",
        return_value=db_path,
    )

    response = client.get("/api/series/non-existent")

    assert response.status_code == 404
    assert response.json()["detail"] == "Series not found"


def test_get_item_by_id(client: TestClient, db_path: Path, mocker):
    """Test GET /api/items/{item_id} returns a single item."""
    mocker.patch(
        "submate.server.handlers.items.router._get_db_path",
        return_value=db_path,
    )

    # Create test item
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
        item_repo.create(
            id="movie-1",
            library_id="lib-1",
            type="movie",
            title="Test Movie",
            path="/media/movies/test.mkv",
            poster_url="/Items/movie-1/Images/Primary",
        )
        subtitle_repo.create(
            item_id="movie-1",
            language="en",
            source="external",
            path="/media/movies/test.en.srt",
        )
        subtitle_repo.create(
            item_id="movie-1",
            language="es",
            source="generated",
            path="/media/movies/test.es.srt",
        )

    response = client.get("/api/items/movie-1")

    assert response.status_code == 200
    data = response.json()
    assert data["id"] == "movie-1"
    assert data["title"] == "Test Movie"
    assert data["type"] == "movie"
    assert data["library_id"] == "lib-1"
    assert data["path"] == "/media/movies/test.mkv"
    assert data["poster_url"] == "/Items/movie-1/Images/Primary"
    assert set(data["subtitle_languages"]) == {"en", "es"}


def test_get_item_not_found(client: TestClient, db_path: Path, mocker):
    """Test GET /api/items/{item_id} returns 404 for non-existent item."""
    mocker.patch(
        "submate.server.handlers.items.router._get_db_path",
        return_value=db_path,
    )

    response = client.get("/api/items/non-existent")

    assert response.status_code == 404
    assert response.json()["detail"] == "Item not found"


def test_get_movies_pagination(client: TestClient, db_path: Path, mocker):
    """Test GET /api/movies with pagination parameters."""
    mocker.patch(
        "submate.server.handlers.items.router._get_db_path",
        return_value=db_path,
    )

    # Create test library and multiple movies
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

        # Create 5 movies
        for i in range(5):
            item_repo.create(
                id=f"movie-{i}",
                library_id="lib-1",
                type="movie",
                title=f"Test Movie {i}",
                path=f"/media/movies/test{i}.mkv",
            )

    # Test page 1 with page_size 2
    response = client.get("/api/movies?page=1&page_size=2")
    assert response.status_code == 200
    data = response.json()
    assert data["total"] == 5
    assert data["page"] == 1
    assert data["page_size"] == 2
    assert len(data["items"]) == 2

    # Test page 2
    response = client.get("/api/movies?page=2&page_size=2")
    assert response.status_code == 200
    data = response.json()
    assert data["total"] == 5
    assert data["page"] == 2
    assert data["page_size"] == 2
    assert len(data["items"]) == 2

    # Test page 3 (partial)
    response = client.get("/api/movies?page=3&page_size=2")
    assert response.status_code == 200
    data = response.json()
    assert data["total"] == 5
    assert data["page"] == 3
    assert data["page_size"] == 2
    assert len(data["items"]) == 1
