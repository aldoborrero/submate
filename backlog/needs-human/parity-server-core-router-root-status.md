# parity: server `GET /` + `GET /status` have no golden, only hand-encoded asserts

**contract:** ROUTE SIGNATURE — `GET /` and `GET /status` response JSON bodies
must match `submate/server/handlers/core/router.py` byte-for-byte (route
paths/signatures are a byte-for-byte contract per the parity brief).

## falsifier

`cargo test -p submate-server parity::core_router` exists and passes against a
golden `rust/fixtures/server/core_router.json` captured from the Python
`create_core_router()` handlers. The golden must pin the exact `GET /` body
(`name`, `version`, `docs`, and the five `endpoints` keys/values) and the
`GET /status` envelope (`status`, `version`, plus the `queue` sub-object key).

## what is missing

The server's `root()` and `status()` handlers
(`rust/crates/submate-server/src/lib.rs`) reproduce the Python `core/router.py`
response shapes, but the only coverage is two inline `#[cfg(test)]` unit tests
(`ops_routes_root_returns_server_info`, `ops_routes_status_has_status_version_queue`)
that **hand-encode** the expected literals:

```rust
// rust/crates/submate-server/src/lib.rs
assert_eq!(body["name"], "Submate Server");
assert_eq!(body["docs"], "/docs");
assert_eq!(body["endpoints"]["status"], "/status");
assert_eq!(body["endpoints"]["queue"], "/queue");
assert_eq!(body["endpoints"]["bazarr_asr"], "/bazarr/asr");
```

These re-state the Python values in Rust rather than asserting against a
captured Python golden, so the Rust handler and the Python SPEC can silently
drift apart without any test failing. This exact class of drift is already
visible elsewhere: `CLAUDE.md` documents the jellyfin route as
`/jellyfin/webhook` while the real Python (`jellyfin/router.py`) and Rust both
use `/webhooks/jellyfin`. A hand-encoded assert offers no protection against
the literals themselves drifting; only a Python-captured golden does.

The root body also omits a coverage guard: the Python `endpoints` dict has
exactly five keys (`bazarr_asr`, `bazarr_detect_language`, `jellyfin`, `status`,
`queue`). The inline test checks only four of them (`bazarr_detect_language`
and `jellyfin` are unchecked), so a missing/renamed `jellyfin` endpoint entry
would pass today.

## where

- Golden capture: add `rust/fixtures/capture/capture_server.py` that imports
  `create_core_router()` (or directly the `root`/`status` coroutine bodies) and
  dumps the two response dicts to `rust/fixtures/server/core_router.json`.
  Note `status` returns `task_queue.stats` under the `queue` key; capture only
  the *static* contract (the `status`/`version`/`queue`-key presence), not the
  live Huey numbers, since the Rust server uses a different node-topology queue
  model on purpose (`{pending,running,done,nodes}` vs Python `{pending,scheduled}`
  — that shape divergence is intentional and out of scope here).
- Test: `rust/crates/submate-server/tests/parity.rs` driving the axum `app(...)`
  router with `tower::ServiceExt::oneshot` for `GET /` and `GET /status`, then
  `parity::assert_json_eq` against the golden (for `/status`, assert the
  `version`/`status` scalars and that a `queue` object key exists; do not pin
  its contents).

## why this is parity, not a port item

The handlers are already implemented and byte-correct against Python today
(`name="Submate Server"`, `version="1.0.0"` == Python `__version__`, all five
endpoint paths match `core/router.py`). The gap is purely test coverage: a
golden-backed falsifier that fails if the Rust root/status response ever drifts
from the Python `core/router.py` SPEC. Out of scope: the `GET /queue`
node-topology stats shape (deliberate re-architecture) and the jellyfin webhook
response body (already filed in `align-jellyfin-webhook-response-shape.md`).

---

**META note (round 2 unpark, 2026-06-12):** re-verified the gate is *phantom*,
not a human/credential gate. This was routed to `needs-human/` as a denylist
"capture-blocked" item, but its capture is **pure-data with no external
runtime** — `submate.server.handlers.core.router` imports cleanly in the nix devshell
(`nix develop --command python3 -c 'import submate.server.handlers.core.router'` succeeds). Per the documented
triage rule in `backlog/meta-contention.md` (pure-data captures → capture
pre-pass runs the capture, item lives in `backlog/`; only external-runtime
captures stay in `needs-human/`), this belongs in `backlog/`. Next round's
capture pre-pass should author `rust/fixtures/capture/capture_core_router.py` and land the golden in a deliberate
capture commit before dispatch — do NOT re-park to `needs-human/`.
