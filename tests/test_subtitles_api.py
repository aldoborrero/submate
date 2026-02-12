"""Tests for Subtitles API endpoints."""

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


@pytest.fixture
def subtitle_dir(tmp_path: Path) -> Path:
    """Create a temporary directory for subtitle files."""
    subs_dir = tmp_path / "subtitles"
    subs_dir.mkdir()
    return subs_dir


def test_list_subtitles(client: TestClient, db_path: Path, mocker):
    """Test GET /api/items/{item_id}/subtitles returns subtitles list."""
    mocker.patch(
        "submate.server.handlers.subtitles.router._get_db_path",
        return_value=db_path,
    )

    # Create test data
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

    response = client.get("/api/items/movie-1/subtitles")

    assert response.status_code == 200
    data = response.json()
    assert data["total"] == 2
    assert len(data["subtitles"]) == 2

    # Check subtitle details
    languages = {sub["language"] for sub in data["subtitles"]}
    assert languages == {"en", "es"}

    # Check all fields are present
    for sub in data["subtitles"]:
        assert "id" in sub
        assert "item_id" in sub
        assert "language" in sub
        assert "source" in sub
        assert "path" in sub
        assert "created_at" in sub


def test_list_subtitles_empty(client: TestClient, db_path: Path, mocker):
    """Test GET /api/items/{item_id}/subtitles returns empty list when no subtitles exist."""
    mocker.patch(
        "submate.server.handlers.subtitles.router._get_db_path",
        return_value=db_path,
    )

    # Create test item with no subtitles
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
            id="movie-1",
            library_id="lib-1",
            type="movie",
            title="Test Movie",
            path="/media/movies/test.mkv",
        )

    response = client.get("/api/items/movie-1/subtitles")

    assert response.status_code == 200
    data = response.json()
    assert data["subtitles"] == []
    assert data["total"] == 0


def test_list_subtitles_item_not_found(client: TestClient, db_path: Path, mocker):
    """Test GET /api/items/{item_id}/subtitles returns 404 for non-existent item."""
    mocker.patch(
        "submate.server.handlers.subtitles.router._get_db_path",
        return_value=db_path,
    )

    response = client.get("/api/items/non-existent/subtitles")

    assert response.status_code == 404
    assert response.json()["detail"] == "Item not found"


def test_get_subtitle_content(client: TestClient, db_path: Path, subtitle_dir: Path, mocker):
    """Test GET /api/items/{item_id}/subtitles/{language} returns subtitle content."""
    mocker.patch(
        "submate.server.handlers.subtitles.router._get_db_path",
        return_value=db_path,
    )

    # Create subtitle file
    subtitle_path = subtitle_dir / "test.en.srt"
    subtitle_content = """1
00:00:01,000 --> 00:00:04,000
Hello, world!

2
00:00:05,000 --> 00:00:08,000
This is a test subtitle.
"""
    subtitle_path.write_text(subtitle_content)

    # Create test data
    with get_db_session(db_path) as session:
        library_repo = LibraryRepository(session)
        item_repo = ItemRepository(session)
        subtitle_repo = SubtitleRepository(session)

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

        subtitle_repo.create(
            item_id="movie-1",
            language="en",
            source="external",
            path=str(subtitle_path),
        )

    response = client.get("/api/items/movie-1/subtitles/en")

    assert response.status_code == 200
    data = response.json()
    assert data["language"] == "en"
    assert data["content"] == subtitle_content
    assert data["format"] == "srt"


def test_get_subtitle_content_ass_format(client: TestClient, db_path: Path, subtitle_dir: Path, mocker):
    """Test GET /api/items/{item_id}/subtitles/{language} detects ASS format."""
    mocker.patch(
        "submate.server.handlers.subtitles.router._get_db_path",
        return_value=db_path,
    )

    # Create ASS subtitle file
    subtitle_path = subtitle_dir / "test.en.ass"
    subtitle_content = """[Script Info]
Title: Test
ScriptType: v4.00+

[V4+ Styles]
Format: Name, Fontname, Fontsize
Style: Default,Arial,20

[Events]
Format: Layer, Start, End, Style, Name, Text
Dialogue: 0,0:00:01.00,0:00:04.00,Default,,Hello world
"""
    subtitle_path.write_text(subtitle_content)

    # Create test data
    with get_db_session(db_path) as session:
        library_repo = LibraryRepository(session)
        item_repo = ItemRepository(session)
        subtitle_repo = SubtitleRepository(session)

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

        subtitle_repo.create(
            item_id="movie-1",
            language="en",
            source="external",
            path=str(subtitle_path),
        )

    response = client.get("/api/items/movie-1/subtitles/en")

    assert response.status_code == 200
    data = response.json()
    assert data["format"] == "ass"


def test_get_subtitle_not_found(client: TestClient, db_path: Path, mocker):
    """Test GET /api/items/{item_id}/subtitles/{language} returns 404 when subtitle not found."""
    mocker.patch(
        "submate.server.handlers.subtitles.router._get_db_path",
        return_value=db_path,
    )

    # Create test item without subtitle
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
            id="movie-1",
            library_id="lib-1",
            type="movie",
            title="Test Movie",
            path="/media/movies/test.mkv",
        )

    response = client.get("/api/items/movie-1/subtitles/en")

    assert response.status_code == 404
    assert response.json()["detail"] == "Subtitle not found"


def test_get_subtitle_item_not_found(client: TestClient, db_path: Path, mocker):
    """Test GET /api/items/{item_id}/subtitles/{language} returns 404 for non-existent item."""
    mocker.patch(
        "submate.server.handlers.subtitles.router._get_db_path",
        return_value=db_path,
    )

    response = client.get("/api/items/non-existent/subtitles/en")

    assert response.status_code == 404
    assert response.json()["detail"] == "Item not found"


def test_save_subtitle_create_new(client: TestClient, db_path: Path, subtitle_dir: Path, mocker):
    """Test PUT /api/items/{item_id}/subtitles/{language} creates new subtitle."""
    mocker.patch(
        "submate.server.handlers.subtitles.router._get_db_path",
        return_value=db_path,
    )

    # Create test item
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
            id="movie-1",
            library_id="lib-1",
            type="movie",
            title="Test Movie",
            path=str(subtitle_dir / "test.mkv"),
        )

    new_content = """1
00:00:01,000 --> 00:00:04,000
New subtitle content!
"""

    response = client.put(
        "/api/items/movie-1/subtitles/en",
        json={"content": new_content},
    )

    assert response.status_code == 200
    data = response.json()
    assert data["item_id"] == "movie-1"
    assert data["language"] == "en"
    assert data["source"] == "generated"
    assert "id" in data
    assert "created_at" in data

    # Verify file was created
    subtitle_path = Path(data["path"])
    assert subtitle_path.exists()
    assert subtitle_path.read_text() == new_content


def test_save_subtitle_update_existing(client: TestClient, db_path: Path, subtitle_dir: Path, mocker):
    """Test PUT /api/items/{item_id}/subtitles/{language} updates existing subtitle."""
    mocker.patch(
        "submate.server.handlers.subtitles.router._get_db_path",
        return_value=db_path,
    )

    # Create existing subtitle file
    subtitle_path = subtitle_dir / "test.en.srt"
    subtitle_path.write_text("Old content")

    # Create test data
    with get_db_session(db_path) as session:
        library_repo = LibraryRepository(session)
        item_repo = ItemRepository(session)
        subtitle_repo = SubtitleRepository(session)

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
            path=str(subtitle_dir / "test.mkv"),
        )

        subtitle_repo.create(
            item_id="movie-1",
            language="en",
            source="external",
            path=str(subtitle_path),
        )

    new_content = """1
00:00:01,000 --> 00:00:04,000
Updated subtitle content!
"""

    response = client.put(
        "/api/items/movie-1/subtitles/en",
        json={"content": new_content},
    )

    assert response.status_code == 200
    data = response.json()
    assert data["item_id"] == "movie-1"
    assert data["language"] == "en"
    # Source should change to 'generated' when edited
    assert data["source"] == "generated"

    # Verify file was updated
    assert subtitle_path.read_text() == new_content


def test_save_subtitle_item_not_found(client: TestClient, db_path: Path, mocker):
    """Test PUT /api/items/{item_id}/subtitles/{language} returns 404 for non-existent item."""
    mocker.patch(
        "submate.server.handlers.subtitles.router._get_db_path",
        return_value=db_path,
    )

    response = client.put(
        "/api/items/non-existent/subtitles/en",
        json={"content": "Some content"},
    )

    assert response.status_code == 404
    assert response.json()["detail"] == "Item not found"


def test_delete_subtitle(client: TestClient, db_path: Path, subtitle_dir: Path, mocker):
    """Test DELETE /api/items/{item_id}/subtitles/{language} deletes subtitle."""
    mocker.patch(
        "submate.server.handlers.subtitles.router._get_db_path",
        return_value=db_path,
    )

    # Create subtitle file
    subtitle_path = subtitle_dir / "test.en.srt"
    subtitle_path.write_text("Content to delete")

    # Create test data
    with get_db_session(db_path) as session:
        library_repo = LibraryRepository(session)
        item_repo = ItemRepository(session)
        subtitle_repo = SubtitleRepository(session)

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

        subtitle_repo.create(
            item_id="movie-1",
            language="en",
            source="external",
            path=str(subtitle_path),
        )

    response = client.delete("/api/items/movie-1/subtitles/en")

    assert response.status_code == 204

    # Verify file was deleted
    assert not subtitle_path.exists()

    # Verify database record was deleted
    with get_db_session(db_path) as session:
        subtitle_repo = SubtitleRepository(session)
        subtitle = subtitle_repo.get_by_item_and_language("movie-1", "en")
        assert subtitle is None


def test_delete_subtitle_not_found(client: TestClient, db_path: Path, mocker):
    """Test DELETE /api/items/{item_id}/subtitles/{language} returns 404 when subtitle not found."""
    mocker.patch(
        "submate.server.handlers.subtitles.router._get_db_path",
        return_value=db_path,
    )

    # Create test item without subtitle
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
            id="movie-1",
            library_id="lib-1",
            type="movie",
            title="Test Movie",
            path="/media/movies/test.mkv",
        )

    response = client.delete("/api/items/movie-1/subtitles/en")

    assert response.status_code == 404
    assert response.json()["detail"] == "Subtitle not found"


def test_delete_subtitle_item_not_found(client: TestClient, db_path: Path, mocker):
    """Test DELETE /api/items/{item_id}/subtitles/{language} returns 404 for non-existent item."""
    mocker.patch(
        "submate.server.handlers.subtitles.router._get_db_path",
        return_value=db_path,
    )

    response = client.delete("/api/items/non-existent/subtitles/en")

    assert response.status_code == 404
    assert response.json()["detail"] == "Item not found"


def test_sync_subtitle(client: TestClient, db_path: Path, subtitle_dir: Path, mocker):
    """Test POST /api/items/{item_id}/subtitles/{language}/sync returns stub response."""
    mocker.patch(
        "submate.server.handlers.subtitles.router._get_db_path",
        return_value=db_path,
    )

    # Create subtitle file
    subtitle_path = subtitle_dir / "test.en.srt"
    subtitle_path.write_text("Content to sync")

    # Create test data
    with get_db_session(db_path) as session:
        library_repo = LibraryRepository(session)
        item_repo = ItemRepository(session)
        subtitle_repo = SubtitleRepository(session)

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

        subtitle_repo.create(
            item_id="movie-1",
            language="en",
            source="external",
            path=str(subtitle_path),
        )

    response = client.post("/api/items/movie-1/subtitles/en/sync")

    assert response.status_code == 200
    data = response.json()
    assert data["success"] is True
    assert "message" in data


def test_sync_subtitle_not_found(client: TestClient, db_path: Path, mocker):
    """Test POST /api/items/{item_id}/subtitles/{language}/sync returns 404 when subtitle not found."""
    mocker.patch(
        "submate.server.handlers.subtitles.router._get_db_path",
        return_value=db_path,
    )

    # Create test item without subtitle
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
            id="movie-1",
            library_id="lib-1",
            type="movie",
            title="Test Movie",
            path="/media/movies/test.mkv",
        )

    response = client.post("/api/items/movie-1/subtitles/en/sync")

    assert response.status_code == 404
    assert response.json()["detail"] == "Subtitle not found"


def test_sync_subtitle_item_not_found(client: TestClient, db_path: Path, mocker):
    """Test POST /api/items/{item_id}/subtitles/{language}/sync returns 404 for non-existent item."""
    mocker.patch(
        "submate.server.handlers.subtitles.router._get_db_path",
        return_value=db_path,
    )

    response = client.post("/api/items/non-existent/subtitles/en/sync")

    assert response.status_code == 404
    assert response.json()["detail"] == "Item not found"
