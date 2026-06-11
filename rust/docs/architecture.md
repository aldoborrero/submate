# submate-rs architecture: server + processing nodes

The Rust port adopts a **FileFlows / Unmanic-style** topology, which is a
deliberate departure from Python submate's single-box server+worker (shared
SQLite). There is **no external broker** — the server owns the queue and
coordinates nodes over HTTP, exactly like FileFlows/Unmanic.

```
                         ┌──────────────────────────────────────────┐
   Bazarr  ──HTTP──▶     │  submate server   (runs where the media is)│
   Jellyfin ─webhook▶    │  • media/audio I/O (ffmpeg)  ── extracts ──┼─▶ PCM
   CLI submit ─HTTP─▶     │  • durable queue (rusqlite, WAL)           │
                         │  • node-coordination API (axum)            │
                         │  • embedded node (default, in-process)     │
                         └─────────────▲───────────────┬──────────────┘
                          pull work    │   ships PCM   │  results/progress
                                       │               ▼
              ┌────────────────────────┴───┐   ┌───────────────────────────┐
              │ submate node  (GPU box)     │   │ submate node  (CPU box)   │
              │ • register(caps: gpu,runners)│  │ • register(caps: cpu)     │
              │ • long-poll request-work     │  │ • translation jobs (LLM)  │
              │ • Dispatcher: Semaphore +    │  │ • Dispatcher (CPU runners)│
              │   spawn_blocking(whisper)    │  └───────────────────────────┘
              └──────────────────────────────┘
```

## Roles

**Server (`submate server`)** — runs where the media is; the brain:
- Owns **all media/audio I/O** (ffprobe/ffmpeg). Nodes never touch media.
- **Central durable queue** in `rusqlite` (bundled, WAL) — stays on the server;
  nodes never open the SQLite file (no NFS-locking footgun).
- **Ingestion → enqueue**: library scanner, Jellyfin webhook, Bazarr ASR
  (high-priority + await result), CLI submit.
- **Node-coordination API** (axum, HTTP):
  - `POST /nodes/register` — node announces `{id, gpu, runners, tasks}` → token.
  - `POST /nodes/{id}/request-work` — **long-poll**; server runs the atomic
    claim filtered by the node's capabilities + priority, returns a job or 204.
  - `GET  /jobs/{id}/audio` — node fetches the extracted PCM payload.
  - `POST /jobs/{id}/progress` · `POST /jobs/{id}/result` — node reports.
  - `POST /nodes/{id}/heartbeat` — keeps the lease alive.
- **Lease reclaim**: jobs whose node went silent (`locked_at + lease < now`)
  return to `queued` — the durability/crash-recovery story.
- **Embedded node** (default): the server runs an in-process node so a single
  box works with no separate process. Disableable for brain-only deployments.

**Node (`submate node --server <url>`)** — GPU box / other machines; stateless compute:
- `register → loop { long-poll request-work → GET audio → Dispatcher → progress
  → result → heartbeat }`.
- **Dispatcher** (lives here, per node): `Semaphore(runners)` + `spawn_blocking`
  into the Whisper model. Per-node concurrency; the GPU node sizes `runners` to
  the GPU, CPU nodes do slower/translation-only work.
- Needs **no ffmpeg / no media access** — the server ships PCM. Translation
  nodes hold LLM credentials.

## Crate placement

| Concern | Crate | Side |
|---|---|---|
| Wire types (register/work/job/result) | `submate-proto` | shared |
| Central durable queue + claim/lease | `submate-queue` | server |
| Node API + ingestion + audio transfer | `submate-server` | server |
| Media I/O (ffmpeg) | `submate-media` | server |
| Node agent + Dispatcher | `submate-node` | node |
| Whisper pipeline | `submate-whisper` + `stable-ts` | node |
| Translation | `submate-translate` | node |

## Job payload & audio transfer

The server is where the media lives, so it extracts audio and the node fetches
it: the `request-work` response carries an `audio_url`; the node does
`GET /jobs/{id}/audio` to pull the PCM (s16le/mono/16k, or f32). For Bazarr,
the uploaded audio is relayed the same way. Large payloads are fetched, never
inlined in JSON.

## Verification

This coordination layer is a **new design**, not a port of Python behavior, so
it is verified **behaviorally / with integration tests**, not against Python
golden fixtures:
- atomic claim (concurrent `request-work` hand out distinct jobs, no double-claim),
- enqueue → claim → result → done lifecycle,
- retry sets `run_at` backoff; lease reclaim resets a stale `running` row,
- capability routing (GPU jobs only to GPU nodes) + priority (Bazarr > scan),
- node agent pull-loop against a mock server; embedded node drains end-to-end.

The **business logic** that IS a faithful port (the 9 transcription skip
conditions, Bazarr output formatting) keeps parity-against-Python falsifiers.
```
