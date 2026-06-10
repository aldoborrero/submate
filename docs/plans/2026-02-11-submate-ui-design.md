# Submate UI Design Document

**Date:** 2026-02-11
**Goal:** Replace Bazarr with a standalone Submate UI for managing Jellyfin library subtitles

---

## Overview

Submate becomes a self-contained subtitle manager for Jellyfin that:
- Syncs library metadata from Jellyfin on startup
- Tracks subtitle status per item per language
- Generates subtitles via Whisper + LLM translation
- Provides a web UI for browsing, bulk operations, and monitoring
- Sends notifications on job completion/failure

---

## Feature 1: YAML Configuration System

### Purpose
Replace environment-variable-only configuration with a human-readable YAML file that the UI can also edit.

### Requirements
- Load settings from `config.yaml` (path configurable via `SUBMATE__CONFIG_PATH` env)
- Environment variables override YAML values (for Docker secrets)
- UI can read and write the config file
- Backward compatible: existing env vars continue to work

### Config Structure

```yaml
jellyfin:
  server_url: "http://jellyfin:8096"
  api_key: ""

libraries:
  - id: ""                    # Auto-populated on sync
    name: ""
    target_languages: ["en"]
    skip_existing: true
    enabled: true

whisper:
  model: "medium"
  device: "cpu"
  compute_type: "int8"

translation:
  backend: "ollama"
  ollama_url: "http://localhost:11434"
  ollama_model: "llama3.2"
  anthropic_api_key: ""
  openai_api_key: ""
  gemini_api_key: ""

notifications:
  on_complete: true
  on_failure: true
  webhooks: []
  ntfy:
    enabled: false
    server: "https://ntfy.sh"
    topic: ""
  apprise:
    enabled: false
    urls: []

database:
  path: ""  # Default: XDG_DATA_HOME/submate/submate.db

server:
  address: "0.0.0.0"
  port: 9000
```

### Implementation

**Files to modify:**
- `submate/config.py` - Add YAML loading with env override

**New files:**
- `submate/config_yaml.py` - YAML read/write utilities

### API Endpoints

| Method | Endpoint | Purpose |
|--------|----------|---------|
| GET | `/api/settings` | Get current configuration |
| PUT | `/api/settings` | Update configuration (writes YAML) |
| POST | `/api/settings/test-jellyfin` | Test Jellyfin connection |

---

## Feature 2: Database Schema

### Purpose
Track Jellyfin library state, subtitle status, and job history in SQLite.

### Requirements
- Store library metadata from Jellyfin
- Track items (movies/episodes) with their subtitle status
- Record job history for debugging and retry

### Schema

```sql
-- Jellyfin libraries
CREATE TABLE libraries (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    type TEXT NOT NULL CHECK (type IN ('movies', 'series')),
    target_languages TEXT NOT NULL DEFAULT '["en"]',  -- JSON array
    skip_existing INTEGER NOT NULL DEFAULT 1,
    enabled INTEGER NOT NULL DEFAULT 1,
    last_synced TEXT
);

-- Media items (movies and episodes)
CREATE TABLE items (
    id TEXT PRIMARY KEY,
    library_id TEXT NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    type TEXT NOT NULL CHECK (type IN ('movie', 'episode')),
    title TEXT NOT NULL,
    path TEXT NOT NULL,
    series_id TEXT,
    series_name TEXT,
    season_num INTEGER,
    episode_num INTEGER,
    poster_url TEXT,
    last_synced TEXT NOT NULL,
    UNIQUE(path)
);

-- Subtitle files (external and generated)
CREATE TABLE subtitles (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    item_id TEXT NOT NULL REFERENCES items(id) ON DELETE CASCADE,
    language TEXT NOT NULL,
    source TEXT NOT NULL CHECK (source IN ('external', 'generated')),
    path TEXT NOT NULL,
    created_at TEXT NOT NULL,
    UNIQUE(item_id, language)
);

-- Job history
CREATE TABLE jobs (
    id TEXT PRIMARY KEY,
    item_id TEXT NOT NULL REFERENCES items(id) ON DELETE CASCADE,
    language TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('pending', 'running', 'completed', 'failed')),
    error TEXT,
    created_at TEXT NOT NULL,
    started_at TEXT,
    completed_at TEXT
);

-- Indexes
CREATE INDEX idx_items_library ON items(library_id);
CREATE INDEX idx_items_series ON items(series_id);
CREATE INDEX idx_subtitles_item ON subtitles(item_id);
CREATE INDEX idx_jobs_item ON jobs(item_id);
CREATE INDEX idx_jobs_status ON jobs(status);
```

### Implementation

**New files:**
- `submate/database/models.py` - SQLAlchemy models
- `submate/database/migrations.py` - Schema creation/migration
- `submate/database/repository.py` - Data access layer

---

## Feature 3: Jellyfin Library Sync

### Purpose
Fetch library metadata from Jellyfin and track it locally for browsing and status tracking.

### Requirements
- Sync all libraries on startup
- Fetch movies and episodes with metadata
- Scan filesystem for existing subtitle files
- Update via webhooks for new content
- Manual re-sync button in UI

### Sync Flow

```
Startup:
1. Load config
2. Connect to Jellyfin
3. GET /Library/VirtualFolders → libraries table
4. For each enabled library:
   - GET /Items (Movies or Series)
   - For Series: GET /Shows/{id}/Episodes
   - Insert/update items table
5. For each item:
   - Scan {path}.*.srt files
   - Insert into subtitles table (source: external)
6. Emit sync.completed event
```

### Webhook Flow

```
ItemAdded webhook:
1. Parse payload for item ID
2. GET /Items/{id} from Jellyfin
3. Insert into items table
4. Scan for existing subtitles
5. If library.enabled and missing target languages:
   - Queue transcription job
6. Emit item.added event
```

### Implementation

**Files to modify:**
- `submate/media_servers/jellyfin.py` - Add library/item fetching

**New files:**
- `submate/services/sync.py` - JellyfinSyncService
- `submate/services/scanner.py` - SubtitleScanner

### API Endpoints

| Method | Endpoint | Purpose |
|--------|----------|---------|
| GET | `/api/libraries` | List libraries with item counts and subtitle stats |
| POST | `/api/libraries/sync` | Trigger full re-sync |
| GET | `/api/libraries/{id}` | Library detail with stats |

---

## Feature 4: Library Browsing API

### Purpose
Provide API endpoints for browsing movies and series with subtitle status.

### Requirements
- Paginated item listing
- Filter by: library, subtitle status, language
- Poster image proxy from Jellyfin
- Series detail with seasons/episodes

### API Endpoints

| Method | Endpoint | Purpose |
|--------|----------|---------|
| GET | `/api/movies` | List movies (paginated) |
| GET | `/api/series` | List series (paginated) |
| GET | `/api/series/{id}` | Series with seasons/episodes |
| GET | `/api/items/{id}` | Single item detail |
| GET | `/api/items/{id}/poster` | Proxy poster from Jellyfin |

### Query Parameters

```
GET /api/movies?library_id=xxx&status=missing&language=es&page=1&limit=50

status: all | missing | complete | partial | failed
language: filter by target language
sort: title | added | status
```

### Response Examples

```json
// GET /api/movies
{
  "items": [
    {
      "id": "abc123",
      "title": "Movie Name (2024)",
      "library_id": "lib1",
      "poster_url": "/api/items/abc123/poster",
      "subtitles": {
        "en": {"status": "generated", "path": "/path/to/movie.en.srt"},
        "es": {"status": "missing"}
      },
      "target_languages": ["en", "es"]
    }
  ],
  "total": 150,
  "page": 1,
  "limit": 50
}

// GET /api/series/xyz789
{
  "id": "xyz789",
  "title": "Series Name",
  "poster_url": "/api/items/xyz789/poster",
  "seasons": [
    {
      "number": 1,
      "episodes": [
        {
          "id": "ep1",
          "episode_num": 1,
          "title": "Pilot",
          "subtitles": {"en": {"status": "generated"}}
        }
      ],
      "stats": {"total": 10, "complete": 8, "missing": 2}
    }
  ],
  "stats": {"total": 50, "complete": 45, "missing": 5}
}
```

### Implementation

**New files:**
- `submate/server/handlers/library/router.py` - Library browsing endpoints
- `submate/server/handlers/library/models.py` - Request/response models

---

## Feature 5: Transcription Queue Management

### Purpose
Queue transcription jobs for individual items or bulk operations.

### Requirements
- Queue single item with target language
- Queue all missing in a library
- Queue filtered selection of items
- Cancel pending jobs
- Retry failed jobs

### API Endpoints

| Method | Endpoint | Purpose |
|--------|----------|---------|
| POST | `/api/items/{id}/transcribe` | Queue single item |
| POST | `/api/libraries/{id}/transcribe` | Queue all missing in library |
| POST | `/api/bulk/transcribe` | Queue selected items |
| GET | `/api/jobs` | List jobs (filterable) |
| GET | `/api/jobs/{id}` | Job detail |
| POST | `/api/jobs/{id}/retry` | Retry failed job |
| DELETE | `/api/jobs/{id}` | Cancel pending job |

### Request Examples

```json
// POST /api/items/{id}/transcribe
{
  "language": "es"  // Optional, uses library default if omitted
}

// POST /api/bulk/transcribe
{
  "item_ids": ["abc", "def", "ghi"],
  "language": "es"  // Optional
}

// GET /api/jobs?status=failed&limit=50
```

### Job State Machine

```
pending → running → completed
              ↓
           failed
              ↓
     (manual retry) → pending
```

### Implementation

**Files to modify:**
- `submate/queue/tasks/transcription.py` - Emit events on state changes

**New files:**
- `submate/server/handlers/jobs/router.py` - Job management endpoints
- `submate/services/job_manager.py` - Job CRUD operations

---

## Feature 6: Real-time Updates (SSE)

### Purpose
Push live updates to the frontend without polling.

### Requirements
- Server-Sent Events stream
- Events for job state changes
- Events for sync completion
- Badge count updates

### Event Types

```
job.started    - Job began processing
job.completed  - Job finished successfully
job.failed     - Job failed with error
sync.completed - Library sync finished
item.added     - New item from webhook
```

### Event Format

```json
event: job.completed
data: {
  "job_id": "uuid",
  "item_id": "abc123",
  "item_title": "Movie Name",
  "language": "es",
  "timestamp": "2024-01-15T10:30:00Z"
}
```

### Implementation

**New files:**
- `submate/server/handlers/events/router.py` - SSE endpoint
- `submate/services/event_bus.py` - Publish/subscribe for events

### API Endpoint

| Method | Endpoint | Purpose |
|--------|----------|---------|
| GET | `/api/events` | SSE stream |

---

## Feature 7: Subtitle Management

### Purpose
View, edit, and manage generated subtitles.

### Requirements
- View subtitle content
- Edit text and timing (basic)
- Bulk time shift
- Delete subtitle
- Trigger ffsubsync on-demand

### API Endpoints

| Method | Endpoint | Purpose |
|--------|----------|---------|
| GET | `/api/items/{id}/subtitles` | List subtitles for item |
| GET | `/api/items/{id}/subtitles/{lang}` | Get subtitle content |
| PUT | `/api/items/{id}/subtitles/{lang}` | Save edited subtitle |
| DELETE | `/api/items/{id}/subtitles/{lang}` | Delete subtitle file |
| POST | `/api/items/{id}/subtitles/{lang}/sync` | Run ffsubsync |

### Request/Response Examples

```json
// GET /api/items/{id}/subtitles/es
{
  "language": "es",
  "source": "generated",
  "path": "/media/movie/movie.es.srt",
  "entries": [
    {"index": 1, "start": "00:00:01,000", "end": "00:00:04,500", "text": "Hola mundo."},
    {"index": 2, "start": "00:00:05,200", "end": "00:00:08,100", "text": "Como estas?"}
  ]
}

// PUT /api/items/{id}/subtitles/es
{
  "entries": [
    {"index": 1, "start": "00:00:01,100", "end": "00:00:04,600", "text": "Hola, mundo."},
    ...
  ]
}

// POST /api/items/{id}/subtitles/es/shift
{
  "offset_ms": 500  // Positive = delay, negative = advance
}
```

### Implementation

**New files:**
- `submate/server/handlers/subtitles/router.py` - Subtitle endpoints
- `submate/services/subtitle_editor.py` - SRT parsing and editing
- `submate/services/ffsubsync.py` - ffsubsync integration

---

## Feature 8: Notifications

### Purpose
Alert users when jobs complete or fail.

### Requirements
- In-app toast notifications (via SSE)
- Generic webhook support
- ntfy integration
- Apprise integration (80+ services)

### Configuration

```yaml
notifications:
  on_complete: true
  on_failure: true
  webhooks:
    - url: "https://example.com/hook"
      events: ["job.completed", "job.failed"]
  ntfy:
    enabled: true
    server: "https://ntfy.sh"
    topic: "submate"
    priority: "default"
  apprise:
    enabled: false
    urls:
      - "discord://webhook_id/token"
```

### Webhook Payload

```json
{
  "event": "job.completed",
  "timestamp": "2024-01-15T10:30:00Z",
  "item": {
    "id": "abc123",
    "title": "Movie Name (2024)",
    "type": "movie"
  },
  "job": {
    "id": "uuid",
    "language": "es",
    "duration_seconds": 145
  }
}
```

### Implementation

**New files:**
- `submate/services/notifications.py` - NotificationService
- `submate/services/notifications/webhook.py` - Generic webhook sender
- `submate/services/notifications/ntfy.py` - ntfy integration
- `submate/services/notifications/apprise.py` - Apprise wrapper

### API Endpoints

| Method | Endpoint | Purpose |
|--------|----------|---------|
| POST | `/api/settings/test-notification` | Test notification config |

---

## Feature 9: Frontend Application

### Purpose
Web UI for browsing library, managing jobs, and configuring settings.

### Tech Stack
- **Runtime:** Bun
- **Framework:** React 18+ with TypeScript
- **Styling:** TBD (Mantine, shadcn/ui, or Tailwind)
- **State:** Plain fetch + React state (upgrade later if needed)
- **Build:** Bun bundler

### Pages

#### Dashboard (`/`)
```
┌─────────────────────────────────────────────────────────────┐
│ Submate                                    [Sync] [Settings]│
├─────────────────────────────────────────────────────────────┤
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐       │
│  │ 1,234    │ │   45     │ │    3     │ │    2     │       │
│  │ Items    │ │ Missing  │ │ Running  │ │ Failed   │       │
│  └──────────┘ └──────────┘ └──────────┘ └──────────┘       │
│                                                             │
│  Recent Activity                                            │
│  ├─ ✓ Movie Name - Spanish generated (2 min ago)           │
│  ├─ ✓ Episode S01E05 - English generated (5 min ago)       │
│  └─ ✗ Another Movie - Failed: timeout (10 min ago)         │
│                                                             │
│  [Process All Missing]                                      │
└─────────────────────────────────────────────────────────────┘
```

#### Movies (`/movies`)
```
┌─────────────────────────────────────────────────────────────┐
│ Movies                          [Filter ▼] [Transcribe (3)] │
├─────────────────────────────────────────────────────────────┤
│ ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐            │
│ │ [poster]│ │ [poster]│ │ [poster]│ │ [poster]│            │
│ │  ✓ en   │ │  ⚠ es   │ │  ✗ err  │ │  ○ --   │            │
│ │Movie 1  │ │Movie 2  │ │Movie 3  │ │Movie 4  │            │
│ └─────────┘ └─────────┘ └─────────┘ └─────────┘            │
│                                                             │
│ Legend: ✓ complete  ⚠ partial  ✗ failed  ○ missing         │
└─────────────────────────────────────────────────────────────┘
```

#### Series (`/series`)
```
┌─────────────────────────────────────────────────────────────┐
│ Series                                     [Filter ▼]       │
├─────────────────────────────────────────────────────────────┤
│ ┌─────────┐ ┌─────────┐ ┌─────────┐                        │
│ │ [poster]│ │ [poster]│ │ [poster]│                        │
│ │ 45/50   │ │ 10/10   │ │  0/24   │                        │
│ │Series 1 │ │Series 2 │ │Series 3 │                        │
│ └─────────┘ └─────────┘ └─────────┘                        │
│                                                             │
│ Click to view episodes                                      │
└─────────────────────────────────────────────────────────────┘
```

#### Series Detail (`/series/:id`)
```
┌─────────────────────────────────────────────────────────────┐
│ ← Back    Series Name                    [Transcribe All]   │
├─────────────────────────────────────────────────────────────┤
│ Season 1 (8/10 complete)                    [Transcribe S1] │
│ ├─ E01 Pilot                    ✓ en  ✓ es                 │
│ ├─ E02 Episode Two              ✓ en  ○ es    [Transcribe] │
│ └─ ...                                                      │
│                                                             │
│ Season 2 (0/12 complete)                    [Transcribe S2] │
│ ├─ E01 New Beginning            ○ en  ○ es    [Transcribe] │
│ └─ ...                                                      │
└─────────────────────────────────────────────────────────────┘
```

#### Queue (`/queue`)
```
┌─────────────────────────────────────────────────────────────┐
│ Queue                                                       │
├─────────────────────────────────────────────────────────────┤
│ [Running 3] [Pending 12] [Completed] [Failed 2]            │
├─────────────────────────────────────────────────────────────┤
│ ┌─────────────────────────────────────────────────────────┐ │
│ │ Movie Name (2024)                           es  ███░░   │ │
│ │ Started 2 minutes ago                                   │ │
│ └─────────────────────────────────────────────────────────┘ │
│ ┌─────────────────────────────────────────────────────────┐ │
│ │ Another Movie                               en  waiting │ │
│ └─────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

#### Settings (`/settings`)
```
┌─────────────────────────────────────────────────────────────┐
│ Settings                                                    │
├─────────────────────────────────────────────────────────────┤
│ [Jellyfin] [Libraries] [Whisper] [Translation] [Notifications]
├─────────────────────────────────────────────────────────────┤
│ Jellyfin Connection                                         │
│                                                             │
│ Server URL:  [http://jellyfin:8096        ]                │
│ API Key:     [••••••••••••••••            ]                │
│                                                             │
│ [Test Connection]                            [Save]         │
└─────────────────────────────────────────────────────────────┘
```

### Component Structure

```
frontend/
├── src/
│   ├── api/              # API client functions
│   ├── components/
│   │   ├── layout/       # Header, Sidebar, Layout
│   │   ├── common/       # Button, Card, Modal, Badge
│   │   ├── movies/       # MovieGrid, MovieCard, MovieModal
│   │   ├── series/       # SeriesGrid, SeriesCard, EpisodeList
│   │   ├── queue/        # JobList, JobCard
│   │   ├── settings/     # SettingsTabs, SettingsForm
│   │   └── subtitles/    # SubtitleEditor, SubtitleTable
│   ├── hooks/
│   │   ├── useSSE.ts     # SSE connection hook
│   │   └── useApi.ts     # Fetch wrapper with loading/error
│   ├── pages/
│   │   ├── Dashboard.tsx
│   │   ├── Movies.tsx
│   │   ├── Series.tsx
│   │   ├── SeriesDetail.tsx
│   │   ├── Queue.tsx
│   │   └── Settings.tsx
│   ├── App.tsx
│   └── main.tsx
├── index.html
├── package.json
└── bunfig.toml
```

---

## Feature 10: ffsubsync Integration

### Purpose
Synchronize subtitle timing to audio on-demand.

### Requirements
- Run ffsubsync on a specific subtitle file
- Queue as a job (can take time)
- Update subtitle file in place
- Report success/failure

### Flow

```
1. User clicks "Sync" on a subtitle
2. POST /api/items/{id}/subtitles/{lang}/sync
3. Job queued with type "sync"
4. Worker runs: ffsubsync video.mkv -i subtitle.srt -o subtitle.srt
5. On success: update subtitles.synced_at
6. Emit job.completed event
```

### Implementation

**New files:**
- `submate/services/ffsubsync.py` - ffsubsync wrapper

**Dependencies:**
- Add `ffsubsync` to optional dependencies in pyproject.toml

---

## Implementation Order

Suggested order based on dependencies:

1. **YAML Configuration** - Foundation for all other features
2. **Database Schema** - Required for tracking state
3. **Jellyfin Library Sync** - Populates the database
4. **Library Browsing API** - Basic read endpoints
5. **Transcription Queue** - Core functionality
6. **Real-time Updates (SSE)** - Better UX for queue
7. **Frontend Application** - User interface
8. **Subtitle Management** - View/edit subtitles
9. **Notifications** - External alerts
10. **ffsubsync Integration** - Optional enhancement

---

## Out of Scope

Explicitly not included in this design:

- Multi-user / authentication
- Plex / Emby support (Jellyfin only)
- External subtitle providers (OpenSubtitles, etc.)
- Full subtitle editor with waveform
- Automatic ffsubsync (on-demand only)
- Automatic retry of failed jobs
- Mobile-specific UI
