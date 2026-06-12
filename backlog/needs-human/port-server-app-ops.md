# axum app + ops routes

**blocked-by:** _(port-queue-store dependency is satisfied — merged 948731f,
retired e3c99b5. This item is no longer dep-blocked; it is parked in needs-human
because the porter tried to author its own denylisted fixture `ops.json`, see
backlog/tried/port-server-app-ops.md.)_

**META note 2026-06-12:** before authoring `ops.json`, reconcile the route
contract. This item's falsifier names `/version` and `/queue/stats`, but the
Python spec (`submate/server/handlers/core/router.py`) exposes `/` (server
info dict), `/status` (`{status, version, queue}`), and `/queue`
(`task_queue.stats`) — no `/version` or `/queue/stats`. A human must decide
whether the Rust port matches Python's route names/shapes or intentionally
diverges, then author `ops.json` to the chosen contract and re-scope the item
back to `backlog/`. Authoring the golden blind would pin the wrong contract.

## what
Build the axum app (feature-flagged routers, global error handler, lifespan) and ops routes `/`, `/version`, `/queue/stats` (pending/running/done from the central queue; plus connected-node count).

## where
`rust/crates/submate-server/src/lib.rs`. `axum` + `tower`.

## why
Server skeleton + health/stats surface.

## falsifies
`cargo test -p submate-server ops_routes` asserts `/version` and `/queue/stats` JSON match `rust/fixtures/server/ops.json`.
