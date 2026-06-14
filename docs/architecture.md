# submate architecture: server + processing nodes

submate uses a **FileFlows / Unmanic-style** topology: there is **no external
broker** — the server owns a durable queue and coordinates processing nodes over
HTTP. A single box works out of the box (the server runs an embedded node);
extra machines join by running `submate node --server <url>`.

```
   Bazarr ──HTTP──▶  ┌──────────────────────────────────────────────┐
   (direct, sync)    │  submate server   (runs where the media is)   │
                     │  • media/audio I/O (ffmpeg)  ── extracts ──────┼─▶ PCM
   CLI submit ─────▶ │  • durable queue (rusqlite, WAL)               │
                     │  • node-coordination API (axum)                │
                     │  • Dispatcher (shared runner cap)              │
                     │  • embedded node (default, in-process)         │
                     └─────────────▲───────────────┬──────────────────┘
                      pull work    │   ships PCM    │  results / progress
                                   │                ▼
              ┌────────────────────┴───────┐   ┌───────────────────────────┐
              │ submate node  (GPU box)     │   │ submate node  (CPU box)   │
              │ • register(caps: gpu,runners)│  │ • register(caps: cpu)     │
              │ • poll request-work          │   │ • Dispatcher (CPU runners)│
              │ • Dispatcher: Semaphore +    │   └───────────────────────────┘
              │   spawn_blocking(whisper)    │
              └──────────────────────────────┘
```

## Two transcription paths

1. **Queue path** (`submate transcribe`, and any future ingestion source) — a
   durable job is enqueued; nodes pull it, fetch the extracted PCM, transcribe,
   and report the result. `transcribe --sync` spins up an in-process coordinator
   + embedded node and waits; `submate node --server <url>` distributes the work
   across machines. This path is durable and crash-recoverable (lease reclaim).

2. **Bazarr path** (`/bazarr/asr`, `/bazarr/detect-language`) — Bazarr's Whisper
   provider is **synchronous** (it holds the connection and reads the subtitle
   from the response body), so this path runs a **direct, semaphore-bounded
   transcription** on the shared `Dispatcher` and returns the result in the
   response — it deliberately **bypasses the durable queue** (a queue can neither
   add durability for an in-RAM upload nor deliver a result after the connection
   drops). It shares the same runner cap as the queue drain, so a whole-show
   burst waits for a runner rather than oversubscribing.

## Roles

**Server (`submate server`)** — runs where the media is:
- Owns **all media/audio I/O** (ffprobe/ffmpeg). Nodes never touch media.
- **Central durable queue** in `rusqlite` (bundled, WAL) — stays on the server;
  nodes never open the SQLite file (no NFS-locking footgun).
- **Node-coordination API** (axum, HTTP):
  - `POST /nodes/register` — node announces `{id, gpu, runners, tasks}` → token.
  - `POST /nodes/{id}/request-work` — atomic, capability-filtered claim; returns
    a job or `204`. The node polls when idle.
  - `GET  /jobs/{id}/audio` — node fetches the extracted PCM payload.
  - `POST /jobs/{id}/progress` · `POST /jobs/{id}/result` — node reports.
  - `POST /nodes/{id}/heartbeat` — keeps the lease alive.
- **Lease reclaim**: jobs whose node went silent (`locked_at + lease < now`)
  return to `queued` — the crash-recovery story.
- **Bazarr direct path** + **embedded node** (default), so a single box works
  with no separate process.

**Node (`submate node --server <url>`)** — stateless compute:
- `register → loop { request-work → GET audio → Dispatcher → progress → result
  → heartbeat }`.
- **Dispatcher**: `Semaphore(runners)` + `spawn_blocking` into Whisper. Per-node
  concurrency; a GPU node sizes `runners` to the GPU.
- Needs **no ffmpeg / no media access** — the server ships PCM. Translation
  nodes hold LLM credentials.

## Crate placement

| Concern | Crate | Side |
|---|---|---|
| Wire types (register/work/job/result) | `submate-proto` | shared |
| Durable queue + claim/lease | `submate-queue` | server |
| Node API + audio transfer + Bazarr direct seam | `submate-server` | server |
| Media I/O (ffmpeg) | `submate-media` | server |
| Node agent + Dispatcher | `submate-node` | node |
| Whisper pipeline | `submate-whisper` + `stable-ts` | node |
| Translation | `submate-translate` | node |

## Audio transfer

The server is where the media lives, so it extracts audio and the node fetches
it: the `request-work` response carries an `audio_url`; the node does
`GET /jobs/{id}/audio` to pull the PCM (s16le/mono/16k). Large payloads are
fetched, never inlined in JSON.

## Verification

The coordination layer is verified **behaviorally / with integration tests**:
- atomic claim (concurrent `request-work` hand out distinct jobs, no double-claim),
- enqueue → claim → result → done lifecycle,
- retry sets `run_at` backoff; lease reclaim resets a stale `running` row,
- capability routing (GPU jobs only to GPU nodes),
- node agent pull-loop against a mock server; embedded node drains end-to-end,
- the Bazarr provider contract (raw-PCM in, SRT-in-body, empty-body-on-failure,
  `200`-`Unknown` on detect failure, shared concurrency cap).

The pure-data business logic (config, language table, paths, stable-ts, subtitle
formatting, mocked-LLM translation) is pinned by the golden fixtures under
`fixtures/` (see `fixtures/README.md`).
```
