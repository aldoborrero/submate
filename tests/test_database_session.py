# tests/test_database_session.py
"""Tests for database session management."""

from pathlib import Path

import pytest
from sqlalchemy import text


def test_get_database_session(tmp_path: Path) -> None:
    """Test creating and using a database session."""
    from submate.database import get_db_session, init_database
    from submate.database.models import Library

    db_path = tmp_path / "test.db"

    # Initialize database first
    init_database(db_path)

    # Use session to create a library
    with get_db_session(db_path) as session:
        library = Library(
            id="test-lib-1",
            name="Test Movies",
            type="movies",
            target_languages=["en", "es"],
            skip_existing=True,
            enabled=True,
        )
        session.add(library)

    # Use another session to verify data was committed
    with get_db_session(db_path) as session:
        result = session.execute(text("SELECT * FROM libraries WHERE id = 'test-lib-1'"))
        row = result.fetchone()
        assert row is not None
        assert row[1] == "Test Movies"  # name column


def test_init_database_creates_tables(tmp_path: Path) -> None:
    """Test that init_database creates all required tables."""
    from sqlalchemy import inspect

    from submate.database import get_db_session, init_database

    db_path = tmp_path / "test_init.db"

    # Initialize database
    init_database(db_path)

    # Check tables were created
    with get_db_session(db_path) as session:
        inspector = inspect(session.get_bind())
        tables = inspector.get_table_names()

        assert "libraries" in tables
        assert "items" in tables
        assert "subtitles" in tables
        assert "jobs" in tables


def test_session_rollback_on_exception(tmp_path: Path) -> None:
    """Test that session rolls back on exception."""
    from submate.database import get_db_session, init_database
    from submate.database.models import Library

    db_path = tmp_path / "test_rollback.db"
    init_database(db_path)

    # Try to add a library but raise an exception
    with pytest.raises(ValueError):
        with get_db_session(db_path) as session:
            library = Library(
                id="rollback-lib",
                name="Should Rollback",
                type="movies",
                target_languages=["en"],
                skip_existing=True,
                enabled=True,
            )
            session.add(library)
            raise ValueError("Simulated error")

    # Verify the library was NOT committed
    with get_db_session(db_path) as session:
        result = session.execute(text("SELECT * FROM libraries WHERE id = 'rollback-lib'"))
        row = result.fetchone()
        assert row is None


def test_init_database_creates_parent_directories(tmp_path: Path) -> None:
    """Test that init_database creates parent directories if needed."""
    from submate.database import init_database

    db_path = tmp_path / "nested" / "path" / "test.db"

    # Parent directories don't exist yet
    assert not db_path.parent.exists()

    # Initialize should create them
    init_database(db_path)

    # Now they should exist
    assert db_path.parent.exists()
    assert db_path.exists()


def test_engine_reuse(tmp_path: Path) -> None:
    """Test that engines are reused for the same database path."""
    from submate.database.session import _get_engine

    db_path = tmp_path / "test_reuse.db"

    # Get engine twice
    engine1 = _get_engine(db_path)
    engine2 = _get_engine(db_path)

    # Should be the same object
    assert engine1 is engine2
