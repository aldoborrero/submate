# axum app + ops routes

**blocked-by:** none (submate-queue store is implemented and merged)

## what
Build the axum app (feature-flagged routers, global error handler, lifespan)
and the **ops routes matching the Python spec** in
`submate/server/handlers/core/router.py`:
- `/` — server info dict,
- `/status` — `{status, version, queue}`,
- `/queue` — queue stats.

Match Python's **route names and the top-level response keys exactly**. The
queue-stats *shape* follows the node topology (see `rust/docs/architecture.md`:
pending/running/done + connected-node count) rather than Huey's
pending/scheduled — so the stats numbers are NOT a Python-golden parity case.

## where
`rust/crates/submate-server/src/lib.rs`. `axum` + `tower`.

## why
Server skeleton + health/stats surface. (Earlier reroute: the prior falsifier
named `/version` and `/queue/stats`, which the Python spec does not expose — the
meta agent correctly refused to pin the wrong contract.)

## falsifies
`cargo test -p submate-server ops_routes` (behavioral, no Python golden):
- `GET /` returns the server-info object,
- `GET /status` returns an object with keys `status`, `version`, `queue` and the
  correct version string (matches `submate.__version__`),
- `GET /queue` returns the node-topology stats object (`pending`, `running`,
  `done`, `nodes`) with zeroed counts on an empty queue.
