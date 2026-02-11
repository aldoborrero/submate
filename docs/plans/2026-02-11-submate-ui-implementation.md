# Submate UI Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a standalone web UI for Submate that replaces Bazarr for managing Jellyfin library subtitles.

**Architecture:** FastAPI backend with SQLite database for state tracking, React frontend with Bun for browsing library and managing transcription jobs. SSE for real-time updates, YAML config file for settings.

**Tech Stack:** Python 3.13, FastAPI, SQLAlchemy, SQLite, Bun, React 18, TypeScript

---

## Phase 1: Backend Foundation

### Task 1: YAML Configuration Loader

**Files:**
- Create: `submate/config_yaml.py`
- Modify: `submate/config.py`
- Test: `tests/test_config_yaml.py`

**Step 1: Write the failing test for YAML loading**

```python
# tests/test_config_yaml.py
import tempfile
from pathlib import Path

import pytest


def test_load_yaml_config_basic():
    """Test loading basic YAML configuration."""
    from submate.config_yaml import load_yaml_config

    yaml_content = """
jellyfin:
  server_url: "http://jellyfin:8096"
  api_key: "test-key"
"""
    with tempfile.NamedTemporaryFile(mode="w", suffix=".yaml", delete=False) as f:
        f.write(yaml_content)
        f.flush()
        config = load_yaml_config(Path(f.name))

    assert config["jellyfin"]["server_url"] == "http://jellyfin:8096"
    assert config["jellyfin"]["api_key"] == "test-key"


def test_load_yaml_config_missing_file_returns_empty():
    """Test that missing file returns empty dict."""
    from submate.config_yaml import load_yaml_config

    config = load_yaml_config(Path("/nonexistent/config.yaml"))
    assert config == {}


def test_save_yaml_config():
    """Test saving configuration to YAML file."""
    from submate.config_yaml import load_yaml_config, save_yaml_config

    config = {
        "jellyfin": {"server_url": "http://localhost:8096", "api_key": "new-key"},
        "whisper": {"model": "large"},
    }

    with tempfile.NamedTemporaryFile(mode="w", suffix=".yaml", delete=False) as f:
        path = Path(f.name)

    save_yaml_config(path, config)
    loaded = load_yaml_config(path)

    assert loaded["jellyfin"]["server_url"] == "http://localhost:8096"
    assert loaded["whisper"]["model"] == "large"
```

**Step 2: Run test to verify it fails**

Run: `pytest tests/test_config_yaml.py -v`
Expected: FAIL with "No module named 'submate.config_yaml'"

**Step 3: Write minimal implementation**

```python
# submate/config_yaml.py
"""YAML configuration file utilities."""

from pathlib import Path
from typing import Any

import yaml


def load_yaml_config(path: Path) -> dict[str, Any]:
    """Load configuration from YAML file.

    Args:
        path: Path to YAML configuration file

    Returns:
        Configuration dictionary, empty if file doesn't exist
    """
    if not path.exists():
        return {}

    with open(path, encoding="utf-8") as f:
        return yaml.safe_load(f) or {}


def save_yaml_config(path: Path, config: dict[str, Any]) -> None:
    """Save configuration to YAML file.

    Args:
        path: Path to YAML configuration file
        config: Configuration dictionary to save
    """
    path.parent.mkdir(parents=True, exist_ok=True)
    with open(path, "w", encoding="utf-8") as f:
        yaml.dump(config, f, default_flow_style=False, sort_keys=False)
```

**Step 4: Run test to verify it passes**

Run: `pytest tests/test_config_yaml.py -v`
Expected: PASS

**Step 5: Commit**

```bash
git add submate/config_yaml.py tests/test_config_yaml.py
git commit -m "feat: add YAML config file loader"
```

---

### Task 2: Database Models

**Files:**
- Create: `submate/database/__init__.py`
- Create: `submate/database/models.py`
- Test: `tests/test_database_models.py`

**Step 1: Write the failing test for database models**

```python
# tests/test_database_models.py
import tempfile
from pathlib import Path

import pytest


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
```

**Step 2: Run test to verify it fails**

Run: `pytest tests/test_database_models.py -v`
Expected: FAIL with "No module named 'submate.database'"

**Step 3: Write minimal implementation**

```python
# submate/database/__init__.py
"""Database package for Submate UI."""

from submate.database.models import Base, Item, Job, Library, Subtitle

__all__ = ["Base", "Library", "Item", "Subtitle", "Job"]
```

```python
# submate/database/models.py
"""SQLAlchemy models for Submate UI database."""

from datetime import datetime
from typing import Optional

from sqlalchemy import Boolean, DateTime, ForeignKey, Index, Integer, String, Text
from sqlalchemy.orm import DeclarativeBase, Mapped, mapped_column, relationship


class Base(DeclarativeBase):
    """Base class for all models."""

    pass


class Library(Base):
    """Jellyfin library metadata."""

    __tablename__ = "libraries"

    id: Mapped[str] = mapped_column(String(64), primary_key=True)
    name: Mapped[str] = mapped_column(String(255), nullable=False)
    type: Mapped[str] = mapped_column(String(20), nullable=False)  # 'movies' or 'series'
    target_languages: Mapped[list[str]] = mapped_column(Text, nullable=False, default="[]")
    skip_existing: Mapped[bool] = mapped_column(Boolean, nullable=False, default=True)
    enabled: Mapped[bool] = mapped_column(Boolean, nullable=False, default=True)
    last_synced: Mapped[Optional[datetime]] = mapped_column(DateTime, nullable=True)

    # Relationships
    items: Mapped[list["Item"]] = relationship("Item", back_populates="library", cascade="all, delete-orphan")

    def __init__(self, **kwargs):
        # Handle target_languages as list
        if "target_languages" in kwargs and isinstance(kwargs["target_languages"], list):
            import json

            kwargs["target_languages"] = json.dumps(kwargs["target_languages"])
        super().__init__(**kwargs)

    @property
    def target_languages_list(self) -> list[str]:
        """Get target languages as list."""
        import json

        if isinstance(self.target_languages, str):
            return json.loads(self.target_languages)
        return self.target_languages


class Item(Base):
    """Media item (movie or episode)."""

    __tablename__ = "items"

    id: Mapped[str] = mapped_column(String(64), primary_key=True)
    library_id: Mapped[str] = mapped_column(String(64), ForeignKey("libraries.id", ondelete="CASCADE"), nullable=False)
    type: Mapped[str] = mapped_column(String(20), nullable=False)  # 'movie' or 'episode'
    title: Mapped[str] = mapped_column(String(500), nullable=False)
    path: Mapped[str] = mapped_column(String(1000), nullable=False, unique=True)
    series_id: Mapped[Optional[str]] = mapped_column(String(64), nullable=True)
    series_name: Mapped[Optional[str]] = mapped_column(String(500), nullable=True)
    season_num: Mapped[Optional[int]] = mapped_column(Integer, nullable=True)
    episode_num: Mapped[Optional[int]] = mapped_column(Integer, nullable=True)
    poster_url: Mapped[Optional[str]] = mapped_column(String(1000), nullable=True)
    last_synced: Mapped[datetime] = mapped_column(DateTime, nullable=False, default=datetime.utcnow)

    # Relationships
    library: Mapped["Library"] = relationship("Library", back_populates="items")
    subtitles: Mapped[list["Subtitle"]] = relationship("Subtitle", back_populates="item", cascade="all, delete-orphan")
    jobs: Mapped[list["Job"]] = relationship("Job", back_populates="item", cascade="all, delete-orphan")

    __table_args__ = (Index("idx_items_library", "library_id"), Index("idx_items_series", "series_id"))


class Subtitle(Base):
    """Subtitle file metadata."""

    __tablename__ = "subtitles"

    id: Mapped[int] = mapped_column(Integer, primary_key=True, autoincrement=True)
    item_id: Mapped[str] = mapped_column(String(64), ForeignKey("items.id", ondelete="CASCADE"), nullable=False)
    language: Mapped[str] = mapped_column(String(10), nullable=False)
    source: Mapped[str] = mapped_column(String(20), nullable=False)  # 'external' or 'generated'
    path: Mapped[str] = mapped_column(String(1000), nullable=False)
    created_at: Mapped[datetime] = mapped_column(DateTime, nullable=False, default=datetime.utcnow)

    # Relationships
    item: Mapped["Item"] = relationship("Item", back_populates="subtitles")

    __table_args__ = (Index("idx_subtitles_item", "item_id"),)


class Job(Base):
    """Transcription job record."""

    __tablename__ = "jobs"

    id: Mapped[str] = mapped_column(String(64), primary_key=True)
    item_id: Mapped[str] = mapped_column(String(64), ForeignKey("items.id", ondelete="CASCADE"), nullable=False)
    language: Mapped[str] = mapped_column(String(10), nullable=False)
    status: Mapped[str] = mapped_column(String(20), nullable=False)  # 'pending', 'running', 'completed', 'failed'
    error: Mapped[Optional[str]] = mapped_column(Text, nullable=True)
    created_at: Mapped[datetime] = mapped_column(DateTime, nullable=False, default=datetime.utcnow)
    started_at: Mapped[Optional[datetime]] = mapped_column(DateTime, nullable=True)
    completed_at: Mapped[Optional[datetime]] = mapped_column(DateTime, nullable=True)

    # Relationships
    item: Mapped["Item"] = relationship("Item", back_populates="jobs")

    __table_args__ = (Index("idx_jobs_item", "item_id"), Index("idx_jobs_status", "status"))
```

**Step 4: Run test to verify it passes**

Run: `pytest tests/test_database_models.py -v`
Expected: PASS

**Step 5: Commit**

```bash
git add submate/database/ tests/test_database_models.py
git commit -m "feat: add SQLAlchemy database models"
```

---

### Task 3: Database Session Management

**Files:**
- Create: `submate/database/session.py`
- Test: `tests/test_database_session.py`

**Step 1: Write the failing test**

```python
# tests/test_database_session.py
import tempfile
from pathlib import Path

import pytest


def test_get_database_session():
    """Test creating database session."""
    from submate.database.session import get_db_session, init_database

    with tempfile.NamedTemporaryFile(suffix=".db", delete=False) as f:
        db_path = Path(f.name)

    init_database(db_path)

    with get_db_session(db_path) as session:
        # Should be able to execute a simple query
        result = session.execute("SELECT 1").fetchone()
        assert result[0] == 1


def test_init_database_creates_tables():
    """Test that init_database creates all tables."""
    from sqlalchemy import create_engine, inspect

    from submate.database.session import init_database

    with tempfile.NamedTemporaryFile(suffix=".db", delete=False) as f:
        db_path = Path(f.name)

    init_database(db_path)

    engine = create_engine(f"sqlite:///{db_path}")
    inspector = inspect(engine)
    tables = inspector.get_table_names()

    assert "libraries" in tables
    assert "items" in tables
```

**Step 2: Run test to verify it fails**

Run: `pytest tests/test_database_session.py -v`
Expected: FAIL with "cannot import name 'get_db_session'"

**Step 3: Write minimal implementation**

```python
# submate/database/session.py
"""Database session management."""

from contextlib import contextmanager
from pathlib import Path
from typing import Generator

from sqlalchemy import create_engine, text
from sqlalchemy.orm import Session, sessionmaker

from submate.database.models import Base

_engines: dict[str, any] = {}
_session_factories: dict[str, sessionmaker] = {}


def init_database(db_path: Path) -> None:
    """Initialize database and create tables.

    Args:
        db_path: Path to SQLite database file
    """
    db_path.parent.mkdir(parents=True, exist_ok=True)
    db_url = f"sqlite:///{db_path}"

    engine = create_engine(db_url, echo=False)
    Base.metadata.create_all(engine)

    _engines[str(db_path)] = engine
    _session_factories[str(db_path)] = sessionmaker(bind=engine)


def _get_engine(db_path: Path):
    """Get or create engine for database path."""
    key = str(db_path)
    if key not in _engines:
        init_database(db_path)
    return _engines[key]


def _get_session_factory(db_path: Path) -> sessionmaker:
    """Get session factory for database path."""
    key = str(db_path)
    if key not in _session_factories:
        init_database(db_path)
    return _session_factories[key]


@contextmanager
def get_db_session(db_path: Path) -> Generator[Session, None, None]:
    """Get database session as context manager.

    Args:
        db_path: Path to SQLite database file

    Yields:
        SQLAlchemy session
    """
    factory = _get_session_factory(db_path)
    session = factory()
    try:
        yield session
        session.commit()
    except Exception:
        session.rollback()
        raise
    finally:
        session.close()
```

**Step 4: Run test to verify it passes**

Run: `pytest tests/test_database_session.py -v`
Expected: PASS

**Step 5: Update __init__.py and commit**

```python
# Update submate/database/__init__.py to include session exports
"""Database package for Submate UI."""

from submate.database.models import Base, Item, Job, Library, Subtitle
from submate.database.session import get_db_session, init_database

__all__ = ["Base", "Library", "Item", "Subtitle", "Job", "get_db_session", "init_database"]
```

```bash
git add submate/database/ tests/test_database_session.py
git commit -m "feat: add database session management"
```

---

### Task 4: Database Repository Layer

**Files:**
- Create: `submate/database/repository.py`
- Test: `tests/test_database_repository.py`

**Step 1: Write the failing test**

```python
# tests/test_database_repository.py
import tempfile
from datetime import datetime
from pathlib import Path

import pytest


@pytest.fixture
def db_path():
    """Create temporary database."""
    from submate.database.session import init_database

    with tempfile.NamedTemporaryFile(suffix=".db", delete=False) as f:
        path = Path(f.name)
    init_database(path)
    return path


def test_create_library(db_path):
    """Test creating a library."""
    from submate.database.repository import LibraryRepository
    from submate.database.session import get_db_session

    with get_db_session(db_path) as session:
        repo = LibraryRepository(session)
        lib = repo.create(
            id="lib1",
            name="Movies",
            type="movies",
            target_languages=["en"],
        )

        assert lib.id == "lib1"
        assert lib.name == "Movies"


def test_get_library_by_id(db_path):
    """Test getting library by ID."""
    from submate.database.repository import LibraryRepository
    from submate.database.session import get_db_session

    with get_db_session(db_path) as session:
        repo = LibraryRepository(session)
        repo.create(id="lib1", name="Movies", type="movies", target_languages=["en"])

    with get_db_session(db_path) as session:
        repo = LibraryRepository(session)
        lib = repo.get_by_id("lib1")

        assert lib is not None
        assert lib.name == "Movies"


def test_create_item(db_path):
    """Test creating an item."""
    from submate.database.repository import ItemRepository, LibraryRepository
    from submate.database.session import get_db_session

    with get_db_session(db_path) as session:
        lib_repo = LibraryRepository(session)
        lib_repo.create(id="lib1", name="Movies", type="movies", target_languages=["en"])

        item_repo = ItemRepository(session)
        item = item_repo.create(
            id="item1",
            library_id="lib1",
            type="movie",
            title="Test Movie",
            path="/media/test.mkv",
        )

        assert item.id == "item1"
        assert item.title == "Test Movie"


def test_list_items_by_library(db_path):
    """Test listing items by library."""
    from submate.database.repository import ItemRepository, LibraryRepository
    from submate.database.session import get_db_session

    with get_db_session(db_path) as session:
        lib_repo = LibraryRepository(session)
        lib_repo.create(id="lib1", name="Movies", type="movies", target_languages=["en"])

        item_repo = ItemRepository(session)
        item_repo.create(id="item1", library_id="lib1", type="movie", title="Movie 1", path="/m1.mkv")
        item_repo.create(id="item2", library_id="lib1", type="movie", title="Movie 2", path="/m2.mkv")

    with get_db_session(db_path) as session:
        item_repo = ItemRepository(session)
        items = item_repo.list_by_library("lib1")

        assert len(items) == 2


def test_create_job(db_path):
    """Test creating a job."""
    from submate.database.repository import ItemRepository, JobRepository, LibraryRepository
    from submate.database.session import get_db_session

    with get_db_session(db_path) as session:
        lib_repo = LibraryRepository(session)
        lib_repo.create(id="lib1", name="Movies", type="movies", target_languages=["en"])

        item_repo = ItemRepository(session)
        item_repo.create(id="item1", library_id="lib1", type="movie", title="Test", path="/t.mkv")

        job_repo = JobRepository(session)
        job = job_repo.create(id="job1", item_id="item1", language="en", status="pending")

        assert job.id == "job1"
        assert job.status == "pending"


def test_list_jobs_by_status(db_path):
    """Test listing jobs by status."""
    from submate.database.repository import ItemRepository, JobRepository, LibraryRepository
    from submate.database.session import get_db_session

    with get_db_session(db_path) as session:
        lib_repo = LibraryRepository(session)
        lib_repo.create(id="lib1", name="Movies", type="movies", target_languages=["en"])

        item_repo = ItemRepository(session)
        item_repo.create(id="item1", library_id="lib1", type="movie", title="Test", path="/t.mkv")

        job_repo = JobRepository(session)
        job_repo.create(id="job1", item_id="item1", language="en", status="pending")
        job_repo.create(id="job2", item_id="item1", language="es", status="completed")

    with get_db_session(db_path) as session:
        job_repo = JobRepository(session)
        pending = job_repo.list_by_status("pending")
        completed = job_repo.list_by_status("completed")

        assert len(pending) == 1
        assert len(completed) == 1
```

**Step 2: Run test to verify it fails**

Run: `pytest tests/test_database_repository.py -v`
Expected: FAIL with "cannot import name 'LibraryRepository'"

**Step 3: Write minimal implementation**

```python
# submate/database/repository.py
"""Repository classes for database operations."""

from datetime import datetime
from typing import Optional

from sqlalchemy.orm import Session

from submate.database.models import Item, Job, Library, Subtitle


class LibraryRepository:
    """Repository for Library operations."""

    def __init__(self, session: Session):
        self.session = session

    def create(
        self,
        id: str,
        name: str,
        type: str,
        target_languages: list[str],
        skip_existing: bool = True,
        enabled: bool = True,
    ) -> Library:
        """Create a new library."""
        lib = Library(
            id=id,
            name=name,
            type=type,
            target_languages=target_languages,
            skip_existing=skip_existing,
            enabled=enabled,
        )
        self.session.add(lib)
        self.session.flush()
        return lib

    def get_by_id(self, id: str) -> Optional[Library]:
        """Get library by ID."""
        return self.session.query(Library).filter(Library.id == id).first()

    def list_all(self) -> list[Library]:
        """List all libraries."""
        return self.session.query(Library).all()

    def update(self, id: str, **kwargs) -> Optional[Library]:
        """Update library fields."""
        lib = self.get_by_id(id)
        if lib:
            for key, value in kwargs.items():
                if hasattr(lib, key):
                    setattr(lib, key, value)
            self.session.flush()
        return lib

    def delete(self, id: str) -> bool:
        """Delete library by ID."""
        lib = self.get_by_id(id)
        if lib:
            self.session.delete(lib)
            return True
        return False


class ItemRepository:
    """Repository for Item operations."""

    def __init__(self, session: Session):
        self.session = session

    def create(
        self,
        id: str,
        library_id: str,
        type: str,
        title: str,
        path: str,
        series_id: Optional[str] = None,
        series_name: Optional[str] = None,
        season_num: Optional[int] = None,
        episode_num: Optional[int] = None,
        poster_url: Optional[str] = None,
    ) -> Item:
        """Create a new item."""
        item = Item(
            id=id,
            library_id=library_id,
            type=type,
            title=title,
            path=path,
            series_id=series_id,
            series_name=series_name,
            season_num=season_num,
            episode_num=episode_num,
            poster_url=poster_url,
            last_synced=datetime.utcnow(),
        )
        self.session.add(item)
        self.session.flush()
        return item

    def get_by_id(self, id: str) -> Optional[Item]:
        """Get item by ID."""
        return self.session.query(Item).filter(Item.id == id).first()

    def list_by_library(
        self, library_id: str, limit: int = 50, offset: int = 0
    ) -> list[Item]:
        """List items by library with pagination."""
        return (
            self.session.query(Item)
            .filter(Item.library_id == library_id)
            .order_by(Item.title)
            .offset(offset)
            .limit(limit)
            .all()
        )

    def list_by_series(self, series_id: str) -> list[Item]:
        """List episodes by series ID."""
        return (
            self.session.query(Item)
            .filter(Item.series_id == series_id)
            .order_by(Item.season_num, Item.episode_num)
            .all()
        )

    def count_by_library(self, library_id: str) -> int:
        """Count items in library."""
        return self.session.query(Item).filter(Item.library_id == library_id).count()

    def upsert(self, id: str, **kwargs) -> Item:
        """Update existing item or create new one."""
        item = self.get_by_id(id)
        if item:
            for key, value in kwargs.items():
                if hasattr(item, key):
                    setattr(item, key, value)
            item.last_synced = datetime.utcnow()
            self.session.flush()
            return item
        return self.create(id=id, **kwargs)


class SubtitleRepository:
    """Repository for Subtitle operations."""

    def __init__(self, session: Session):
        self.session = session

    def create(
        self, item_id: str, language: str, source: str, path: str
    ) -> Subtitle:
        """Create a new subtitle record."""
        sub = Subtitle(
            item_id=item_id,
            language=language,
            source=source,
            path=path,
            created_at=datetime.utcnow(),
        )
        self.session.add(sub)
        self.session.flush()
        return sub

    def get_by_item_and_language(
        self, item_id: str, language: str
    ) -> Optional[Subtitle]:
        """Get subtitle by item ID and language."""
        return (
            self.session.query(Subtitle)
            .filter(Subtitle.item_id == item_id, Subtitle.language == language)
            .first()
        )

    def list_by_item(self, item_id: str) -> list[Subtitle]:
        """List subtitles for item."""
        return self.session.query(Subtitle).filter(Subtitle.item_id == item_id).all()

    def delete(self, id: int) -> bool:
        """Delete subtitle by ID."""
        sub = self.session.query(Subtitle).filter(Subtitle.id == id).first()
        if sub:
            self.session.delete(sub)
            return True
        return False

    def upsert(self, item_id: str, language: str, source: str, path: str) -> Subtitle:
        """Update or create subtitle."""
        sub = self.get_by_item_and_language(item_id, language)
        if sub:
            sub.source = source
            sub.path = path
            self.session.flush()
            return sub
        return self.create(item_id, language, source, path)


class JobRepository:
    """Repository for Job operations."""

    def __init__(self, session: Session):
        self.session = session

    def create(
        self, id: str, item_id: str, language: str, status: str = "pending"
    ) -> Job:
        """Create a new job."""
        job = Job(
            id=id,
            item_id=item_id,
            language=language,
            status=status,
            created_at=datetime.utcnow(),
        )
        self.session.add(job)
        self.session.flush()
        return job

    def get_by_id(self, id: str) -> Optional[Job]:
        """Get job by ID."""
        return self.session.query(Job).filter(Job.id == id).first()

    def list_by_status(
        self, status: str, limit: int = 50, offset: int = 0
    ) -> list[Job]:
        """List jobs by status."""
        return (
            self.session.query(Job)
            .filter(Job.status == status)
            .order_by(Job.created_at.desc())
            .offset(offset)
            .limit(limit)
            .all()
        )

    def list_by_item(self, item_id: str) -> list[Job]:
        """List jobs for item."""
        return (
            self.session.query(Job)
            .filter(Job.item_id == item_id)
            .order_by(Job.created_at.desc())
            .all()
        )

    def update_status(
        self, id: str, status: str, error: Optional[str] = None
    ) -> Optional[Job]:
        """Update job status."""
        job = self.get_by_id(id)
        if job:
            job.status = status
            if status == "running":
                job.started_at = datetime.utcnow()
            elif status in ("completed", "failed"):
                job.completed_at = datetime.utcnow()
            if error:
                job.error = error
            self.session.flush()
        return job

    def count_by_status(self, status: str) -> int:
        """Count jobs by status."""
        return self.session.query(Job).filter(Job.status == status).count()
```

**Step 4: Run test to verify it passes**

Run: `pytest tests/test_database_repository.py -v`
Expected: PASS

**Step 5: Update __init__.py and commit**

```python
# Update submate/database/__init__.py
"""Database package for Submate UI."""

from submate.database.models import Base, Item, Job, Library, Subtitle
from submate.database.repository import (
    ItemRepository,
    JobRepository,
    LibraryRepository,
    SubtitleRepository,
)
from submate.database.session import get_db_session, init_database

__all__ = [
    "Base",
    "Library",
    "Item",
    "Subtitle",
    "Job",
    "get_db_session",
    "init_database",
    "LibraryRepository",
    "ItemRepository",
    "SubtitleRepository",
    "JobRepository",
]
```

```bash
git add submate/database/ tests/test_database_repository.py
git commit -m "feat: add database repository layer"
```

---

### Task 5: Event Bus for Real-time Updates

**Files:**
- Create: `submate/services/__init__.py`
- Create: `submate/services/event_bus.py`
- Test: `tests/test_event_bus.py`

**Step 1: Write the failing test**

```python
# tests/test_event_bus.py
import asyncio

import pytest


def test_event_bus_subscribe_and_publish():
    """Test subscribing to and publishing events."""
    from submate.services.event_bus import EventBus

    bus = EventBus()
    received = []

    def handler(event):
        received.append(event)

    bus.subscribe("test.event", handler)
    bus.publish("test.event", {"data": "value"})

    assert len(received) == 1
    assert received[0]["data"] == "value"


def test_event_bus_multiple_subscribers():
    """Test multiple subscribers receive events."""
    from submate.services.event_bus import EventBus

    bus = EventBus()
    received1 = []
    received2 = []

    bus.subscribe("test.event", lambda e: received1.append(e))
    bus.subscribe("test.event", lambda e: received2.append(e))
    bus.publish("test.event", {"data": "value"})

    assert len(received1) == 1
    assert len(received2) == 1


def test_event_bus_unsubscribe():
    """Test unsubscribing from events."""
    from submate.services.event_bus import EventBus

    bus = EventBus()
    received = []

    def handler(event):
        received.append(event)

    sub_id = bus.subscribe("test.event", handler)
    bus.publish("test.event", {"data": "first"})
    bus.unsubscribe("test.event", sub_id)
    bus.publish("test.event", {"data": "second"})

    assert len(received) == 1


def test_event_bus_global_instance():
    """Test global event bus instance."""
    from submate.services.event_bus import get_event_bus

    bus1 = get_event_bus()
    bus2 = get_event_bus()

    assert bus1 is bus2
```

**Step 2: Run test to verify it fails**

Run: `pytest tests/test_event_bus.py -v`
Expected: FAIL with "No module named 'submate.services'"

**Step 3: Write minimal implementation**

```python
# submate/services/__init__.py
"""Services package for Submate."""

from submate.services.event_bus import EventBus, get_event_bus

__all__ = ["EventBus", "get_event_bus"]
```

```python
# submate/services/event_bus.py
"""Event bus for real-time updates."""

import logging
import uuid
from collections import defaultdict
from dataclasses import dataclass
from datetime import datetime
from typing import Any, Callable

logger = logging.getLogger(__name__)

EventHandler = Callable[[dict[str, Any]], None]


@dataclass
class Event:
    """Event data structure."""

    type: str
    data: dict[str, Any]
    timestamp: datetime


class EventBus:
    """Simple pub/sub event bus for internal events."""

    def __init__(self):
        self._subscribers: dict[str, dict[str, EventHandler]] = defaultdict(dict)

    def subscribe(self, event_type: str, handler: EventHandler) -> str:
        """Subscribe to an event type.

        Args:
            event_type: Event type to subscribe to
            handler: Callback function to handle events

        Returns:
            Subscription ID for unsubscribing
        """
        sub_id = str(uuid.uuid4())
        self._subscribers[event_type][sub_id] = handler
        logger.debug(f"Subscribed to {event_type}: {sub_id}")
        return sub_id

    def unsubscribe(self, event_type: str, sub_id: str) -> bool:
        """Unsubscribe from an event type.

        Args:
            event_type: Event type to unsubscribe from
            sub_id: Subscription ID from subscribe()

        Returns:
            True if unsubscribed, False if not found
        """
        if sub_id in self._subscribers[event_type]:
            del self._subscribers[event_type][sub_id]
            logger.debug(f"Unsubscribed from {event_type}: {sub_id}")
            return True
        return False

    def publish(self, event_type: str, data: dict[str, Any]) -> None:
        """Publish an event to all subscribers.

        Args:
            event_type: Event type to publish
            data: Event data
        """
        event = Event(type=event_type, data=data, timestamp=datetime.utcnow())
        handlers = list(self._subscribers[event_type].values())

        logger.debug(f"Publishing {event_type} to {len(handlers)} subscribers")

        for handler in handlers:
            try:
                handler(data)
            except Exception as e:
                logger.error(f"Error in event handler for {event_type}: {e}")


# Global instance
_event_bus: EventBus | None = None


def get_event_bus() -> EventBus:
    """Get global event bus instance."""
    global _event_bus
    if _event_bus is None:
        _event_bus = EventBus()
    return _event_bus
```

**Step 4: Run test to verify it passes**

Run: `pytest tests/test_event_bus.py -v`
Expected: PASS

**Step 5: Commit**

```bash
git add submate/services/ tests/test_event_bus.py
git commit -m "feat: add event bus for real-time updates"
```

---

## Phase 2: Jellyfin Integration

### Task 6: Extended Jellyfin Client

**Files:**
- Modify: `submate/media_servers/jellyfin.py`
- Test: `tests/test_jellyfin_extended.py`

**Step 1: Write the failing test**

```python
# tests/test_jellyfin_extended.py
import pytest
from unittest.mock import Mock, patch


def test_get_libraries():
    """Test fetching libraries from Jellyfin."""
    from submate.config import Config
    from submate.media_servers.jellyfin import JellyfinClient

    config = Mock(spec=Config)
    config.jellyfin.server_url = "http://jellyfin:8096"
    config.jellyfin.api_key = "test-key"
    config.jellyfin.libraries = []

    client = JellyfinClient(config)
    client.server_url = config.jellyfin.server_url
    client.api_key = config.jellyfin.api_key

    mock_response = Mock()
    mock_response.json.return_value = [
        {"Id": "lib1", "Name": "Movies", "CollectionType": "movies"},
        {"Id": "lib2", "Name": "TV Shows", "CollectionType": "tvshows"},
    ]
    mock_response.raise_for_status = Mock()

    with patch("requests.get", return_value=mock_response):
        libraries = client.get_libraries()

    assert len(libraries) == 2
    assert libraries[0]["Id"] == "lib1"
    assert libraries[0]["Name"] == "Movies"


def test_get_library_items():
    """Test fetching items from a library."""
    from submate.config import Config
    from submate.media_servers.jellyfin import JellyfinClient

    config = Mock(spec=Config)
    config.jellyfin.server_url = "http://jellyfin:8096"
    config.jellyfin.api_key = "test-key"
    config.jellyfin.libraries = []

    client = JellyfinClient(config)
    client.server_url = config.jellyfin.server_url
    client.api_key = config.jellyfin.api_key
    client._admin_user_id = "admin1"

    mock_response = Mock()
    mock_response.json.return_value = {
        "Items": [
            {"Id": "movie1", "Name": "Test Movie", "Path": "/media/movie.mkv", "Type": "Movie"},
        ],
        "TotalRecordCount": 1,
    }
    mock_response.raise_for_status = Mock()

    with patch("requests.get", return_value=mock_response):
        result = client.get_library_items("lib1", item_type="Movie")

    assert result["TotalRecordCount"] == 1
    assert result["Items"][0]["Name"] == "Test Movie"


def test_get_series_episodes():
    """Test fetching episodes for a series."""
    from submate.config import Config
    from submate.media_servers.jellyfin import JellyfinClient

    config = Mock(spec=Config)
    config.jellyfin.server_url = "http://jellyfin:8096"
    config.jellyfin.api_key = "test-key"
    config.jellyfin.libraries = []

    client = JellyfinClient(config)
    client.server_url = config.jellyfin.server_url
    client.api_key = config.jellyfin.api_key
    client._admin_user_id = "admin1"

    mock_response = Mock()
    mock_response.json.return_value = {
        "Items": [
            {
                "Id": "ep1",
                "Name": "Pilot",
                "IndexNumber": 1,
                "ParentIndexNumber": 1,
                "Path": "/media/show/s01e01.mkv",
            },
        ],
        "TotalRecordCount": 1,
    }
    mock_response.raise_for_status = Mock()

    with patch("requests.get", return_value=mock_response):
        result = client.get_series_episodes("series1")

    assert result["TotalRecordCount"] == 1
    assert result["Items"][0]["Name"] == "Pilot"
```

**Step 2: Run test to verify it fails**

Run: `pytest tests/test_jellyfin_extended.py -v`
Expected: FAIL with "AttributeError: 'JellyfinClient' object has no attribute 'get_libraries'"

**Step 3: Add methods to JellyfinClient**

Add the following methods to `submate/media_servers/jellyfin.py`:

```python
    def get_libraries(self) -> list[dict]:
        """Get all Jellyfin libraries.

        Returns:
            List of library dictionaries with Id, Name, CollectionType
        """
        if not self.server_url or not self.api_key:
            raise RuntimeError("Not connected to Jellyfin server")

        headers = {"X-MediaBrowser-Token": self.api_key}
        response = requests.get(
            f"{self.server_url}/Library/VirtualFolders",
            headers=headers,
            timeout=10,
        )
        response.raise_for_status()
        return response.json()

    def get_library_items(
        self,
        library_id: str,
        item_type: str = "Movie",
        start_index: int = 0,
        limit: int = 100,
    ) -> dict:
        """Get items from a library.

        Args:
            library_id: Jellyfin library ID
            item_type: Item type filter (Movie, Series, Episode)
            start_index: Pagination start index
            limit: Maximum items to return

        Returns:
            Dict with Items list and TotalRecordCount
        """
        if not self.server_url or not self.api_key:
            raise RuntimeError("Not connected to Jellyfin server")

        admin_id = self._get_admin_user_id()
        headers = {"Authorization": f"MediaBrowser Token={self.api_key}"}

        params = {
            "ParentId": library_id,
            "IncludeItemTypes": item_type,
            "Recursive": "true",
            "StartIndex": start_index,
            "Limit": limit,
            "Fields": "Path,Overview,DateCreated",
            "SortBy": "SortName",
            "SortOrder": "Ascending",
        }

        response = requests.get(
            f"{self.server_url}/Users/{admin_id}/Items",
            headers=headers,
            params=params,
            timeout=30,
        )
        response.raise_for_status()
        return response.json()

    def get_series_episodes(self, series_id: str) -> dict:
        """Get all episodes for a series.

        Args:
            series_id: Jellyfin series ID

        Returns:
            Dict with Items list and TotalRecordCount
        """
        if not self.server_url or not self.api_key:
            raise RuntimeError("Not connected to Jellyfin server")

        admin_id = self._get_admin_user_id()
        headers = {"Authorization": f"MediaBrowser Token={self.api_key}"}

        params = {
            "Fields": "Path,Overview,DateCreated",
            "SortBy": "ParentIndexNumber,IndexNumber",
            "SortOrder": "Ascending",
        }

        response = requests.get(
            f"{self.server_url}/Shows/{series_id}/Episodes",
            headers=headers,
            params=params,
            timeout=30,
        )
        response.raise_for_status()
        return response.json()

    def get_item(self, item_id: str) -> dict:
        """Get single item details.

        Args:
            item_id: Jellyfin item ID

        Returns:
            Item dictionary with full details
        """
        if not self.server_url or not self.api_key:
            raise RuntimeError("Not connected to Jellyfin server")

        admin_id = self._get_admin_user_id()
        headers = {"Authorization": f"MediaBrowser Token={self.api_key}"}

        response = requests.get(
            f"{self.server_url}/Users/{admin_id}/Items/{item_id}",
            headers=headers,
            timeout=10,
        )
        response.raise_for_status()
        return response.json()

    def get_poster_url(self, item_id: str) -> str:
        """Get poster image URL for an item.

        Args:
            item_id: Jellyfin item ID

        Returns:
            Full URL to poster image
        """
        return f"{self.server_url}/Items/{item_id}/Images/Primary"
```

**Step 4: Run test to verify it passes**

Run: `pytest tests/test_jellyfin_extended.py -v`
Expected: PASS

**Step 5: Commit**

```bash
git add submate/media_servers/jellyfin.py tests/test_jellyfin_extended.py
git commit -m "feat: extend Jellyfin client with library browsing"
```

---

### Task 7: Subtitle Scanner Service

**Files:**
- Create: `submate/services/scanner.py`
- Test: `tests/test_scanner.py`

**Step 1: Write the failing test**

```python
# tests/test_scanner.py
import tempfile
from pathlib import Path

import pytest


def test_scan_subtitles_finds_srt():
    """Test scanning finds .srt files next to media."""
    from submate.services.scanner import SubtitleScanner

    with tempfile.TemporaryDirectory() as tmpdir:
        media_path = Path(tmpdir) / "movie.mkv"
        sub_en = Path(tmpdir) / "movie.en.srt"
        sub_es = Path(tmpdir) / "movie.es.srt"

        media_path.touch()
        sub_en.write_text("1\n00:00:01,000 --> 00:00:02,000\nHello")
        sub_es.write_text("1\n00:00:01,000 --> 00:00:02,000\nHola")

        scanner = SubtitleScanner()
        subs = scanner.scan_for_media(media_path)

        assert len(subs) == 2
        assert "en" in [s["language"] for s in subs]
        assert "es" in [s["language"] for s in subs]


def test_scan_subtitles_detects_language_from_filename():
    """Test language detection from filename patterns."""
    from submate.services.scanner import SubtitleScanner

    scanner = SubtitleScanner()

    assert scanner.detect_language_from_filename("movie.en.srt") == "en"
    assert scanner.detect_language_from_filename("movie.eng.srt") == "en"
    assert scanner.detect_language_from_filename("movie.spanish.srt") == "es"
    assert scanner.detect_language_from_filename("movie.srt") is None


def test_scan_subtitles_handles_no_subtitles():
    """Test scanning returns empty list when no subtitles."""
    from submate.services.scanner import SubtitleScanner

    with tempfile.TemporaryDirectory() as tmpdir:
        media_path = Path(tmpdir) / "movie.mkv"
        media_path.touch()

        scanner = SubtitleScanner()
        subs = scanner.scan_for_media(media_path)

        assert len(subs) == 0
```

**Step 2: Run test to verify it fails**

Run: `pytest tests/test_scanner.py -v`
Expected: FAIL with "cannot import name 'SubtitleScanner'"

**Step 3: Write minimal implementation**

```python
# submate/services/scanner.py
"""Subtitle file scanner."""

import logging
import re
from pathlib import Path
from typing import Optional

logger = logging.getLogger(__name__)

# Language code mappings
LANGUAGE_CODES = {
    # ISO 639-1
    "en": "en",
    "es": "es",
    "fr": "fr",
    "de": "de",
    "it": "it",
    "pt": "pt",
    "ja": "ja",
    "ko": "ko",
    "zh": "zh",
    "ru": "ru",
    "ar": "ar",
    "hi": "hi",
    "nl": "nl",
    "pl": "pl",
    "sv": "sv",
    "da": "da",
    "no": "no",
    "fi": "fi",
    # ISO 639-2
    "eng": "en",
    "spa": "es",
    "fra": "fr",
    "fre": "fr",
    "deu": "de",
    "ger": "de",
    "ita": "it",
    "por": "pt",
    "jpn": "ja",
    "kor": "ko",
    "zho": "zh",
    "chi": "zh",
    "rus": "ru",
    "ara": "ar",
    "hin": "hi",
    "nld": "nl",
    "dut": "nl",
    "pol": "pl",
    "swe": "sv",
    "dan": "da",
    "nor": "no",
    "fin": "fi",
    # Full names
    "english": "en",
    "spanish": "es",
    "french": "fr",
    "german": "de",
    "italian": "it",
    "portuguese": "pt",
    "japanese": "ja",
    "korean": "ko",
    "chinese": "zh",
    "russian": "ru",
    "arabic": "ar",
    "hindi": "hi",
    "dutch": "nl",
    "polish": "pl",
    "swedish": "sv",
    "danish": "da",
    "norwegian": "no",
    "finnish": "fi",
}

SUBTITLE_EXTENSIONS = {".srt", ".ass", ".ssa", ".sub", ".vtt"}


class SubtitleScanner:
    """Scans filesystem for subtitle files."""

    def detect_language_from_filename(self, filename: str) -> Optional[str]:
        """Detect language code from subtitle filename.

        Handles patterns like:
        - movie.en.srt
        - movie.eng.srt
        - movie.english.srt

        Args:
            filename: Subtitle filename

        Returns:
            ISO 639-1 language code or None if not detected
        """
        path = Path(filename)
        stem = path.stem  # movie.en for movie.en.srt

        # Try to extract language from the last part before extension
        parts = stem.split(".")
        if len(parts) >= 2:
            lang_part = parts[-1].lower()
            if lang_part in LANGUAGE_CODES:
                return LANGUAGE_CODES[lang_part]

        return None

    def scan_for_media(self, media_path: Path) -> list[dict]:
        """Scan for subtitle files next to a media file.

        Args:
            media_path: Path to media file

        Returns:
            List of dicts with language, path, and source info
        """
        if not media_path.exists():
            return []

        parent = media_path.parent
        base_name = media_path.stem

        subtitles = []

        # Look for subtitle files matching the media name
        for ext in SUBTITLE_EXTENSIONS:
            # Pattern: movie.*.srt or movie.srt
            for sub_path in parent.glob(f"{base_name}*{ext}"):
                language = self.detect_language_from_filename(sub_path.name)

                # If no language detected and it's exact match (movie.srt), mark as unknown
                if language is None and sub_path.stem == base_name:
                    language = "und"  # Undetermined

                if language:
                    subtitles.append(
                        {
                            "language": language,
                            "path": str(sub_path),
                            "source": "external",
                        }
                    )

        logger.debug(f"Found {len(subtitles)} subtitles for {media_path.name}")
        return subtitles

    def scan_directory(self, directory: Path) -> dict[str, list[dict]]:
        """Scan entire directory for media files and their subtitles.

        Args:
            directory: Directory to scan

        Returns:
            Dict mapping media paths to subtitle lists
        """
        media_extensions = {".mkv", ".mp4", ".avi", ".mov", ".wmv", ".m4v"}
        results = {}

        for ext in media_extensions:
            for media_path in directory.rglob(f"*{ext}"):
                subs = self.scan_for_media(media_path)
                if subs:
                    results[str(media_path)] = subs

        return results
```

**Step 4: Run test to verify it passes**

Run: `pytest tests/test_scanner.py -v`
Expected: PASS

**Step 5: Update services __init__.py and commit**

```python
# Update submate/services/__init__.py
"""Services package for Submate."""

from submate.services.event_bus import EventBus, get_event_bus
from submate.services.scanner import SubtitleScanner

__all__ = ["EventBus", "get_event_bus", "SubtitleScanner"]
```

```bash
git add submate/services/ tests/test_scanner.py
git commit -m "feat: add subtitle file scanner"
```

---

### Task 8: Jellyfin Sync Service

**Files:**
- Create: `submate/services/sync.py`
- Test: `tests/test_sync_service.py`

**Step 1: Write the failing test**

```python
# tests/test_sync_service.py
import tempfile
from pathlib import Path
from unittest.mock import Mock, patch

import pytest


@pytest.fixture
def db_path():
    """Create temporary database."""
    from submate.database.session import init_database

    with tempfile.NamedTemporaryFile(suffix=".db", delete=False) as f:
        path = Path(f.name)
    init_database(path)
    return path


@pytest.fixture
def mock_jellyfin_client():
    """Create mock Jellyfin client."""
    client = Mock()
    client.get_libraries.return_value = [
        {"Id": "lib1", "Name": "Movies", "CollectionType": "movies"},
        {"Id": "lib2", "Name": "TV Shows", "CollectionType": "tvshows"},
    ]
    client.get_library_items.return_value = {
        "Items": [
            {"Id": "movie1", "Name": "Test Movie", "Path": "/media/test.mkv", "Type": "Movie"},
        ],
        "TotalRecordCount": 1,
    }
    client.get_poster_url.return_value = "http://jellyfin/Items/movie1/Images/Primary"
    return client


def test_sync_libraries(db_path, mock_jellyfin_client):
    """Test syncing libraries from Jellyfin."""
    from submate.database.repository import LibraryRepository
    from submate.database.session import get_db_session
    from submate.services.sync import JellyfinSyncService

    sync_service = JellyfinSyncService(mock_jellyfin_client, db_path)
    sync_service.sync_libraries()

    with get_db_session(db_path) as session:
        repo = LibraryRepository(session)
        libs = repo.list_all()

        assert len(libs) == 2
        assert libs[0].name == "Movies"


def test_sync_library_items(db_path, mock_jellyfin_client):
    """Test syncing items from a library."""
    from submate.database.repository import ItemRepository, LibraryRepository
    from submate.database.session import get_db_session
    from submate.services.sync import JellyfinSyncService

    # First sync libraries
    sync_service = JellyfinSyncService(mock_jellyfin_client, db_path)
    sync_service.sync_libraries()

    # Then sync items
    with patch.object(sync_service.scanner, "scan_for_media", return_value=[]):
        sync_service.sync_library_items("lib1")

    with get_db_session(db_path) as session:
        repo = ItemRepository(session)
        items = repo.list_by_library("lib1")

        assert len(items) == 1
        assert items[0].title == "Test Movie"
```

**Step 2: Run test to verify it fails**

Run: `pytest tests/test_sync_service.py -v`
Expected: FAIL with "cannot import name 'JellyfinSyncService'"

**Step 3: Write minimal implementation**

```python
# submate/services/sync.py
"""Jellyfin library sync service."""

import logging
from datetime import datetime
from pathlib import Path
from typing import TYPE_CHECKING

from submate.database.repository import (
    ItemRepository,
    LibraryRepository,
    SubtitleRepository,
)
from submate.database.session import get_db_session
from submate.services.event_bus import get_event_bus
from submate.services.scanner import SubtitleScanner

if TYPE_CHECKING:
    from submate.media_servers.jellyfin import JellyfinClient

logger = logging.getLogger(__name__)


class JellyfinSyncService:
    """Service for syncing Jellyfin library to local database."""

    def __init__(self, jellyfin_client: "JellyfinClient", db_path: Path):
        self.jellyfin = jellyfin_client
        self.db_path = db_path
        self.scanner = SubtitleScanner()
        self.event_bus = get_event_bus()

    def sync_libraries(self) -> list[dict]:
        """Sync all libraries from Jellyfin.

        Returns:
            List of synced library info
        """
        logger.info("Syncing libraries from Jellyfin")
        libraries = self.jellyfin.get_libraries()
        synced = []

        with get_db_session(self.db_path) as session:
            repo = LibraryRepository(session)

            for lib in libraries:
                lib_type = self._map_collection_type(lib.get("CollectionType", ""))
                if lib_type:
                    existing = repo.get_by_id(lib["Id"])
                    if existing:
                        repo.update(
                            lib["Id"],
                            name=lib["Name"],
                            type=lib_type,
                            last_synced=datetime.utcnow(),
                        )
                    else:
                        repo.create(
                            id=lib["Id"],
                            name=lib["Name"],
                            type=lib_type,
                            target_languages=["en"],  # Default
                        )
                    synced.append({"id": lib["Id"], "name": lib["Name"], "type": lib_type})

        logger.info(f"Synced {len(synced)} libraries")
        return synced

    def sync_library_items(self, library_id: str) -> int:
        """Sync all items from a library.

        Args:
            library_id: Jellyfin library ID

        Returns:
            Number of items synced
        """
        logger.info(f"Syncing items for library {library_id}")

        with get_db_session(self.db_path) as session:
            lib_repo = LibraryRepository(session)
            library = lib_repo.get_by_id(library_id)
            if not library:
                raise ValueError(f"Library {library_id} not found")

            lib_type = library.type

        # Determine item type based on library type
        if lib_type == "movies":
            item_type = "Movie"
        else:
            item_type = "Series"

        # Fetch items from Jellyfin
        total_synced = 0
        start_index = 0
        limit = 100

        while True:
            result = self.jellyfin.get_library_items(
                library_id, item_type=item_type, start_index=start_index, limit=limit
            )
            items = result.get("Items", [])

            if not items:
                break

            with get_db_session(self.db_path) as session:
                item_repo = ItemRepository(session)
                sub_repo = SubtitleRepository(session)

                for item in items:
                    self._sync_item(item, library_id, lib_type, item_repo, sub_repo)
                    total_synced += 1

            start_index += limit
            if start_index >= result.get("TotalRecordCount", 0):
                break

        # For series libraries, also sync episodes
        if lib_type == "series":
            total_synced += self._sync_all_episodes(library_id)

        logger.info(f"Synced {total_synced} items for library {library_id}")
        return total_synced

    def _sync_item(
        self,
        item: dict,
        library_id: str,
        lib_type: str,
        item_repo: ItemRepository,
        sub_repo: SubtitleRepository,
    ) -> None:
        """Sync a single item and its subtitles."""
        item_type = "movie" if lib_type == "movies" else "series"

        item_repo.upsert(
            id=item["Id"],
            library_id=library_id,
            type=item_type,
            title=item["Name"],
            path=item.get("Path", ""),
            poster_url=self.jellyfin.get_poster_url(item["Id"]),
        )

        # Scan for subtitles if item has a path
        if item.get("Path"):
            media_path = Path(item["Path"])
            subtitles = self.scanner.scan_for_media(media_path)
            for sub in subtitles:
                sub_repo.upsert(
                    item_id=item["Id"],
                    language=sub["language"],
                    source=sub["source"],
                    path=sub["path"],
                )

    def _sync_all_episodes(self, library_id: str) -> int:
        """Sync episodes for all series in a library."""
        total = 0

        # Get all series
        with get_db_session(self.db_path) as session:
            item_repo = ItemRepository(session)
            series_list = item_repo.list_by_library(library_id, limit=10000)

        for series in series_list:
            if series.type == "series":
                total += self._sync_series_episodes(series.id, library_id)

        return total

    def _sync_series_episodes(self, series_id: str, library_id: str) -> int:
        """Sync episodes for a single series."""
        result = self.jellyfin.get_series_episodes(series_id)
        episodes = result.get("Items", [])

        with get_db_session(self.db_path) as session:
            item_repo = ItemRepository(session)
            sub_repo = SubtitleRepository(session)

            # Get series name
            series = item_repo.get_by_id(series_id)
            series_name = series.title if series else ""

            for ep in episodes:
                item_repo.upsert(
                    id=ep["Id"],
                    library_id=library_id,
                    type="episode",
                    title=ep["Name"],
                    path=ep.get("Path", ""),
                    series_id=series_id,
                    series_name=series_name,
                    season_num=ep.get("ParentIndexNumber"),
                    episode_num=ep.get("IndexNumber"),
                    poster_url=self.jellyfin.get_poster_url(ep["Id"]),
                )

                # Scan for subtitles
                if ep.get("Path"):
                    media_path = Path(ep["Path"])
                    subtitles = self.scanner.scan_for_media(media_path)
                    for sub in subtitles:
                        sub_repo.upsert(
                            item_id=ep["Id"],
                            language=sub["language"],
                            source=sub["source"],
                            path=sub["path"],
                        )

        return len(episodes)

    def sync_all(self) -> dict:
        """Sync all libraries and their items.

        Returns:
            Summary of synced data
        """
        libraries = self.sync_libraries()
        total_items = 0

        for lib in libraries:
            count = self.sync_library_items(lib["id"])
            total_items += count

        summary = {
            "libraries": len(libraries),
            "items": total_items,
            "timestamp": datetime.utcnow().isoformat(),
        }

        self.event_bus.publish("sync.completed", summary)
        return summary

    @staticmethod
    def _map_collection_type(collection_type: str) -> str | None:
        """Map Jellyfin collection type to our type."""
        mapping = {
            "movies": "movies",
            "tvshows": "series",
            "music": None,  # Not supported
            "musicvideos": None,
            "books": None,
        }
        return mapping.get(collection_type)
```

**Step 4: Run test to verify it passes**

Run: `pytest tests/test_sync_service.py -v`
Expected: PASS

**Step 5: Update services __init__.py and commit**

```python
# Update submate/services/__init__.py
"""Services package for Submate."""

from submate.services.event_bus import EventBus, get_event_bus
from submate.services.scanner import SubtitleScanner
from submate.services.sync import JellyfinSyncService

__all__ = ["EventBus", "get_event_bus", "SubtitleScanner", "JellyfinSyncService"]
```

```bash
git add submate/services/ tests/test_sync_service.py
git commit -m "feat: add Jellyfin sync service"
```

---

## Phase 3: API Endpoints

### Task 9: Library API Endpoints

**Files:**
- Create: `submate/server/handlers/library/__init__.py`
- Create: `submate/server/handlers/library/router.py`
- Create: `submate/server/handlers/library/models.py`
- Test: `tests/test_library_api.py`

**Step 1: Write the failing test**

```python
# tests/test_library_api.py
import tempfile
from pathlib import Path

import pytest
from fastapi.testclient import TestClient


@pytest.fixture
def db_path():
    """Create temporary database."""
    from submate.database.session import init_database

    with tempfile.NamedTemporaryFile(suffix=".db", delete=False) as f:
        path = Path(f.name)
    init_database(path)
    return path


@pytest.fixture
def test_client(db_path, monkeypatch):
    """Create test client with mocked database path."""
    monkeypatch.setenv("SUBMATE__DATABASE__PATH", str(db_path))

    from submate.server.server import create_app

    app = create_app()
    return TestClient(app)


def test_get_libraries_empty(test_client):
    """Test GET /api/libraries returns empty list."""
    response = test_client.get("/api/libraries")
    assert response.status_code == 200
    assert response.json() == {"libraries": [], "total": 0}


def test_get_libraries_with_data(test_client, db_path):
    """Test GET /api/libraries returns libraries."""
    from submate.database.repository import LibraryRepository
    from submate.database.session import get_db_session

    # Add test data
    with get_db_session(db_path) as session:
        repo = LibraryRepository(session)
        repo.create(id="lib1", name="Movies", type="movies", target_languages=["en"])

    response = test_client.get("/api/libraries")
    assert response.status_code == 200
    data = response.json()
    assert data["total"] == 1
    assert data["libraries"][0]["name"] == "Movies"
```

**Step 2: Run test to verify it fails**

Run: `pytest tests/test_library_api.py -v`
Expected: FAIL (endpoint doesn't exist)

**Step 3: Write minimal implementation**

```python
# submate/server/handlers/library/__init__.py
"""Library API handlers."""

from submate.server.handlers.library.router import router

__all__ = ["router"]
```

```python
# submate/server/handlers/library/models.py
"""Pydantic models for library API."""

from datetime import datetime
from typing import Optional

from pydantic import BaseModel


class LibraryResponse(BaseModel):
    """Single library response."""

    id: str
    name: str
    type: str
    target_languages: list[str]
    skip_existing: bool
    enabled: bool
    last_synced: Optional[datetime] = None
    item_count: int = 0
    subtitle_stats: dict = {}


class LibraryListResponse(BaseModel):
    """Library list response."""

    libraries: list[LibraryResponse]
    total: int


class LibraryUpdateRequest(BaseModel):
    """Request to update library settings."""

    target_languages: Optional[list[str]] = None
    skip_existing: Optional[bool] = None
    enabled: Optional[bool] = None


class SyncResponse(BaseModel):
    """Sync operation response."""

    libraries: int
    items: int
    timestamp: str
```

```python
# submate/server/handlers/library/router.py
"""Library API router."""

import json
import logging
from pathlib import Path

from fastapi import APIRouter, HTTPException

from submate.config import get_config
from submate.database.repository import ItemRepository, LibraryRepository, SubtitleRepository
from submate.database.session import get_db_session
from submate.server.handlers.library.models import (
    LibraryListResponse,
    LibraryResponse,
    LibraryUpdateRequest,
    SyncResponse,
)

logger = logging.getLogger(__name__)
router = APIRouter(prefix="/api/libraries", tags=["libraries"])


def _get_db_path() -> Path:
    """Get database path from config."""
    config = get_config()
    return Path(config.queue.db_path).parent / "submate.db"


@router.get("", response_model=LibraryListResponse)
async def list_libraries():
    """List all libraries with stats."""
    db_path = _get_db_path()

    with get_db_session(db_path) as session:
        lib_repo = LibraryRepository(session)
        item_repo = ItemRepository(session)
        sub_repo = SubtitleRepository(session)

        libraries = lib_repo.list_all()
        result = []

        for lib in libraries:
            item_count = item_repo.count_by_library(lib.id)

            # Parse target_languages
            if isinstance(lib.target_languages, str):
                target_langs = json.loads(lib.target_languages)
            else:
                target_langs = lib.target_languages

            result.append(
                LibraryResponse(
                    id=lib.id,
                    name=lib.name,
                    type=lib.type,
                    target_languages=target_langs,
                    skip_existing=lib.skip_existing,
                    enabled=lib.enabled,
                    last_synced=lib.last_synced,
                    item_count=item_count,
                )
            )

    return LibraryListResponse(libraries=result, total=len(result))


@router.get("/{library_id}", response_model=LibraryResponse)
async def get_library(library_id: str):
    """Get single library details."""
    db_path = _get_db_path()

    with get_db_session(db_path) as session:
        lib_repo = LibraryRepository(session)
        item_repo = ItemRepository(session)

        lib = lib_repo.get_by_id(library_id)
        if not lib:
            raise HTTPException(status_code=404, detail="Library not found")

        item_count = item_repo.count_by_library(lib.id)

        if isinstance(lib.target_languages, str):
            target_langs = json.loads(lib.target_languages)
        else:
            target_langs = lib.target_languages

        return LibraryResponse(
            id=lib.id,
            name=lib.name,
            type=lib.type,
            target_languages=target_langs,
            skip_existing=lib.skip_existing,
            enabled=lib.enabled,
            last_synced=lib.last_synced,
            item_count=item_count,
        )


@router.patch("/{library_id}", response_model=LibraryResponse)
async def update_library(library_id: str, update: LibraryUpdateRequest):
    """Update library settings."""
    db_path = _get_db_path()

    with get_db_session(db_path) as session:
        lib_repo = LibraryRepository(session)
        item_repo = ItemRepository(session)

        lib = lib_repo.get_by_id(library_id)
        if not lib:
            raise HTTPException(status_code=404, detail="Library not found")

        update_data = update.model_dump(exclude_none=True)
        if "target_languages" in update_data:
            update_data["target_languages"] = json.dumps(update_data["target_languages"])

        lib_repo.update(library_id, **update_data)
        lib = lib_repo.get_by_id(library_id)
        item_count = item_repo.count_by_library(lib.id)

        if isinstance(lib.target_languages, str):
            target_langs = json.loads(lib.target_languages)
        else:
            target_langs = lib.target_languages

        return LibraryResponse(
            id=lib.id,
            name=lib.name,
            type=lib.type,
            target_languages=target_langs,
            skip_existing=lib.skip_existing,
            enabled=lib.enabled,
            last_synced=lib.last_synced,
            item_count=item_count,
        )
```

**Step 4: Register router in server.py**

Modify `submate/server/server.py` to include the new router:

```python
# Add import at top
from submate.server.handlers.library import router as library_router

# In create_app(), add:
app.include_router(library_router)
```

**Step 5: Run test to verify it passes**

Run: `pytest tests/test_library_api.py -v`
Expected: PASS

**Step 6: Commit**

```bash
git add submate/server/handlers/library/ submate/server/server.py tests/test_library_api.py
git commit -m "feat: add library API endpoints"
```

---

## Remaining Tasks (Summary)

Due to the size of this implementation plan, the remaining tasks are summarized below. Each follows the same TDD pattern:

### Task 10: Items API Endpoints
- `GET /api/movies` - List movies with pagination and filtering
- `GET /api/series` - List series with pagination
- `GET /api/series/{id}` - Series detail with episodes
- `GET /api/items/{id}` - Single item detail
- `GET /api/items/{id}/poster` - Proxy poster from Jellyfin

### Task 11: Jobs API Endpoints
- `POST /api/items/{id}/transcribe` - Queue single item
- `POST /api/libraries/{id}/transcribe` - Queue all missing
- `POST /api/bulk/transcribe` - Queue selected items
- `GET /api/jobs` - List jobs
- `POST /api/jobs/{id}/retry` - Retry failed
- `DELETE /api/jobs/{id}` - Cancel pending

### Task 12: SSE Events Endpoint
- `GET /api/events` - Server-Sent Events stream
- Emit events on job state changes
- Emit events on sync completion

### Task 13: Subtitles API Endpoints
- `GET /api/items/{id}/subtitles` - List subtitles
- `GET /api/items/{id}/subtitles/{lang}` - Get content
- `PUT /api/items/{id}/subtitles/{lang}` - Save edited
- `DELETE /api/items/{id}/subtitles/{lang}` - Delete
- `POST /api/items/{id}/subtitles/{lang}/sync` - ffsubsync

### Task 14: Settings API Endpoints
- `GET /api/settings` - Get config
- `PUT /api/settings` - Update config
- `POST /api/settings/test-jellyfin` - Test connection
- `POST /api/settings/test-notification` - Test notifications

### Task 15: Notification Service
- Webhook sender
- ntfy integration
- Apprise integration
- Hook into job events

## Phase 4: Frontend

### Task 16: Frontend Scaffolding
- Initialize Bun project
- Setup React + TypeScript
- Configure build for integration with FastAPI

### Task 17: API Client
- TypeScript API client matching backend endpoints
- Error handling
- Type definitions from OpenAPI

### Task 18: Layout Components
- Header with navigation
- Sidebar
- Main layout wrapper

### Task 19: Dashboard Page
- Stats cards
- Recent activity
- Quick actions

### Task 20: Movies Page
- Grid view with posters
- Filtering and selection
- Transcribe action

### Task 21: Series Pages
- Series grid
- Series detail with episodes
- Season/episode selection

### Task 22: Queue Page
- Job list with tabs
- Real-time updates via SSE
- Retry/cancel actions

### Task 23: Settings Page
- Tabbed interface
- Forms for each section
- Test buttons

### Task 24: Subtitle Editor
- Table view of entries
- Inline editing
- Time shift controls

---

## Execution Checklist

- [ ] Task 1: YAML Configuration Loader
- [ ] Task 2: Database Models
- [ ] Task 3: Database Session Management
- [ ] Task 4: Database Repository Layer
- [ ] Task 5: Event Bus
- [ ] Task 6: Extended Jellyfin Client
- [ ] Task 7: Subtitle Scanner Service
- [ ] Task 8: Jellyfin Sync Service
- [ ] Task 9: Library API Endpoints
- [ ] Task 10: Items API Endpoints
- [ ] Task 11: Jobs API Endpoints
- [ ] Task 12: SSE Events Endpoint
- [ ] Task 13: Subtitles API Endpoints
- [ ] Task 14: Settings API Endpoints
- [ ] Task 15: Notification Service
- [ ] Task 16: Frontend Scaffolding
- [ ] Task 17: API Client
- [ ] Task 18: Layout Components
- [ ] Task 19: Dashboard Page
- [ ] Task 20: Movies Page
- [ ] Task 21: Series Pages
- [ ] Task 22: Queue Page
- [ ] Task 23: Settings Page
- [ ] Task 24: Subtitle Editor
