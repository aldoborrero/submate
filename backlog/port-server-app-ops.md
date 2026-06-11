# axum app + ops routes

**blocked-by:** port-queue-store

## what
Build the axum app (feature-flagged routers, global error handler, lifespan) and ops routes `/`, `/version`, `/queue/stats` (pending/running/done from the central queue; plus connected-node count).

## where
`rust/crates/submate-server/src/lib.rs`. `axum` + `tower`.

## why
Server skeleton + health/stats surface.

## falsifies
`cargo test -p submate-server ops_routes` asserts `/version` and `/queue/stats` JSON match `rust/fixtures/server/ops.json`.
