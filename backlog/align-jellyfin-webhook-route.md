# align: Jellyfin webhook route is `/webhooks/jellyfin`, NOT `/jellyfin/webhook`

**contract:** ROUTE SIGNATURE — route paths are a byte-for-byte contract. The
Rust server must mount the Jellyfin webhook at the *same* path Python's FastAPI
app exposes it. Python's actual path is `POST /webhooks/jellyfin`.

This item is referenced as a dependency by `backlog/port-server-jellyfin-webhook.md`
("see `backlog/align-jellyfin-webhook-route.md`") and
`backlog/parity-server-core-router-root-status.md`, but the align file was never
filed. It is filed here to make the contract authoritative and falsifiable so a
future implementer cannot "correct" the Rust route to the wrong path based on a
drifted SPEC doc.

## the drift

The Python SPEC truth (`submate/server/handlers/jellyfin/router.py`):

```python
router = APIRouter(prefix="/webhooks", tags=["jellyfin"])

@router.post("/jellyfin")
async def jellyfin_webhook(...): ...
```

→ effective path **`POST /webhooks/jellyfin`**.

The root endpoint advertises the same value
(`submate/server/handlers/core/router.py`, the `endpoints` dict):

```python
"jellyfin": "/webhooks/jellyfin",
```

`README.md` is correct (`/webhooks/jellyfin`, lines 137 + 154). The Rust port is
already correct: `rust/crates/submate-server/src/lib.rs`
(`jellyfin_router()` mounts `.route("/webhooks/jellyfin", post(jellyfin_webhook))`,
and `root()`'s `endpoints.jellyfin` = `"/webhooks/jellyfin"`).

The drifted artifact is **`CLAUDE.md`** — the project's own instruction file —
which documents the wrong path in two places:

| location | drifted text | should be |
|----------|--------------|-----------|
| `CLAUDE.md:217` (endpoint table) | `\| `/jellyfin/webhook` \| POST \| Jellyfin event webhook \|` | `/webhooks/jellyfin` |
| `CLAUDE.md:232` (Jellyfin Integration step) | ``Add webhook URL: `http://submate:9000/jellyfin/webhook` `` | `…/webhooks/jellyfin` |

The grind round brief itself repeats this wrong path (`/jellyfin/webhook`) in
its route enumeration, so the misinformation propagates from `CLAUDE.md`. A
future server/CLI/integration implementer who trusts `CLAUDE.md` over the actual
`jellyfin/router.py` would register `/jellyfin/webhook`, silently breaking every
real Jellyfin webhook plugin POST (404), with no test to catch it because the
existing Rust coverage hand-encodes the literal rather than diffing a golden.

## fix

1. Correct both `CLAUDE.md` occurrences (lines 217 + 232) to `/webhooks/jellyfin`
   so the project SPEC doc matches the Python router and `README.md`.
2. Pin the route-path contract with a golden so it cannot drift silently. Capture
   the *registered route paths* from the Python app and diff the Rust router
   against them (not just an inline literal assert):
   - Capture: `rust/fixtures/capture/capture_server_routes.py` — build the app
     (or each router factory) and dump the sorted `(method, path)` pairs for the
     contract routes (`GET /`, `GET /status`, `GET /queue`, `POST /bazarr/asr`,
     `POST /bazarr/detect-language`, `POST /webhooks/jellyfin`) to
     `rust/fixtures/server/routes.json`. This import is pure-data (no external
     runtime): `python3 -c 'import submate.server.handlers.jellyfin.router'`
     succeeds in the nix devshell.
   - Test: in `rust/crates/submate-server/tests/parity.rs`, assert the axum
     `app(...)` answers each golden `(method, path)` with a non-404 status and
     404s on the *wrong* `/jellyfin/webhook` (negative guard), via
     `tower::ServiceExt::oneshot`.

## falsifies

`cargo test -p submate-server parity::routes` — `POST /webhooks/jellyfin` is
routed (non-404) and `POST /jellyfin/webhook` is **not** (404), and the captured
golden `rust/fixtures/server/routes.json` lists `/webhooks/jellyfin` (never
`/jellyfin/webhook`). Plus: `grep -n '/jellyfin/webhook' CLAUDE.md` returns
nothing (all occurrences corrected to `/webhooks/jellyfin`).
