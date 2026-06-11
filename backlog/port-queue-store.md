# Central durable job store (rusqlite, WAL)

**blocked-by:** port-config-schema, port-proto-types

## what
Hand-roll the server's central queue: a `jobs` table (id, kind, payload, state queued|running|done|failed, attempts, max_attempts, run_at, locked_by, locked_at), enqueue, atomic claim (`UPDATE ... WHERE id=(SELECT ... LIMIT 1) RETURNING *`), complete/fail with exponential-backoff `run_at`, and startup **lease reclaim** of stale `running` rows. See rust/docs/architecture.md. NOT a Huey clone, NOT apalis.

## where
`rust/crates/submate-queue/src/lib.rs`. `rusqlite` (bundled) + WAL + busy_timeout.

## why
The coordination substrate the whole server↔node system runs on; stays local to the server (nodes pull over HTTP, never touch SQLite).

## falsifies
`cargo test -p submate-queue claim_atomic` — N concurrent claims return distinct jobs (no double-claim); queued→running→done lifecycle; a failed job re-queues with backoff `run_at`; a stale `running` row (lease expired) is reclaimed to `queued`.
