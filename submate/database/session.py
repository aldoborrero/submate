"""Database session management for Submate UI."""

from collections.abc import Generator
from contextlib import contextmanager
from pathlib import Path

from sqlalchemy import Engine, create_engine
from sqlalchemy.orm import Session, sessionmaker

from submate.database.models import Base

# Module-level caches for engines and session factories
_engines: dict[str, Engine] = {}
_session_factories: dict[str, sessionmaker[Session]] = {}


def _get_engine(db_path: Path) -> Engine:
    """Get or create a SQLAlchemy engine for the given database path.

    Args:
        db_path: Path to the SQLite database file.

    Returns:
        SQLAlchemy Engine instance.
    """
    path_str = str(db_path.resolve())
    if path_str not in _engines:
        _engines[path_str] = create_engine(f"sqlite:///{path_str}")
    return _engines[path_str]


def _get_session_factory(db_path: Path) -> sessionmaker[Session]:
    """Get or create a session factory for the given database path.

    Args:
        db_path: Path to the SQLite database file.

    Returns:
        SQLAlchemy sessionmaker instance.
    """
    path_str = str(db_path.resolve())
    if path_str not in _session_factories:
        engine = _get_engine(db_path)
        _session_factories[path_str] = sessionmaker(bind=engine)
    return _session_factories[path_str]


def init_database(db_path: Path) -> None:
    """Initialize the database and create all tables.

    Creates parent directories if they don't exist.

    Args:
        db_path: Path to the SQLite database file.
    """
    # Create parent directories if needed
    db_path.parent.mkdir(parents=True, exist_ok=True)

    # Get or create engine
    engine = _get_engine(db_path)

    # Create all tables
    Base.metadata.create_all(engine)


@contextmanager
def get_db_session(db_path: Path) -> Generator[Session]:
    """Context manager that yields a SQLAlchemy session.

    Auto-commits on success, rolls back on exception, and closes session in finally.

    Args:
        db_path: Path to the SQLite database file.

    Yields:
        SQLAlchemy Session instance.
    """
    session_factory = _get_session_factory(db_path)
    session = session_factory()
    try:
        yield session
        session.commit()
    except Exception:
        session.rollback()
        raise
    finally:
        session.close()
