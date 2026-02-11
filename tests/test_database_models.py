# tests/test_database_models.py
"""Tests for SQLAlchemy database models."""

import tempfile


def test_create_tables():
    """Test that all tables are created correctly."""
    from sqlalchemy import create_engine, inspect

    from submate.database.models import Base

    with tempfile.NamedTemporaryFile(suffix=".db", delete=False) as f:
        db_path = f.name

    engine = create_engine(f"sqlite:///{db_path}")
    Base.metadata.create_all(engine)

    inspector = inspect(engine)
    tables = inspector.get_table_names()

    assert "libraries" in tables
    assert "items" in tables
    assert "subtitles" in tables
    assert "jobs" in tables


def test_library_model():
    """Test Library model creation."""
    from submate.database.models import Library

    lib = Library(
        id="abc123",
        name="Movies",
        type="movies",
        target_languages=["en", "es"],
        skip_existing=True,
        enabled=True,
    )

    assert lib.id == "abc123"
    assert lib.name == "Movies"
    assert lib.target_languages == ["en", "es"]


def test_item_model():
    """Test Item model creation."""
    from submate.database.models import Item

    item = Item(
        id="item1",
        library_id="lib1",
        type="movie",
        title="Test Movie",
        path="/media/movies/test.mkv",
    )

    assert item.id == "item1"
    assert item.type == "movie"


def test_subtitle_model():
    """Test Subtitle model creation."""
    from submate.database.models import Subtitle

    sub = Subtitle(
        item_id="item1",
        language="en",
        source="generated",
        path="/media/movies/test.en.srt",
    )

    assert sub.language == "en"
    assert sub.source == "generated"


def test_job_model():
    """Test Job model creation."""
    from submate.database.models import Job

    job = Job(
        id="job1",
        item_id="item1",
        language="es",
        status="pending",
    )

    assert job.status == "pending"
