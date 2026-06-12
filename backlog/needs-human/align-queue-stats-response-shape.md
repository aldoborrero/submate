# align: GET /queue response shape is {pending, running, done, nodes}, Python emits {pending, scheduled}

> **META note (needs-human re-triage): genuine design gate, NOT a phantom denylist
> gate.** The abandon (`fb6bb0d`) was triggered by a grind attempt rewriting the
> committed denylisted golden `rust/fixtures/server/core_router.json` — but that
> golden already exists and is correct; the real blocker is the "Decision needed"
> section below. A human must pick option 1 (Python-compat view) or option 2
> (sanctioned contract delta) before this is dispatchable. Re-running the grind
> only reproduces the abandon. Do NOT auto-unpark this the way fixture-gated items
> get unparked; it needs the decision first. Once decided, rewrite the body into a
> concrete port task and move it back to `backlog/`.

## Contract

ROUTE SIGNATURES — request/response JSON shapes must match Python
byte-for-byte. The drifting route is `GET /queue` (and the `queue` value
embedded in `GET /status`).

## Python SPEC

`submate/server/handlers/core/router.py`:

- `GET /queue` (`queue_status`) returns `task_queue.stats` verbatim.
- `GET /status` (`status`) returns `{"status": "ok", "version": __version__,
  "queue": task_queue.stats}`.

`submate/queue/task_queue.py` `TaskQueue.stats` (property) returns **exactly two
keys**:

```python
return {
    "pending": pending if isinstance(pending, int) else 0,
    "scheduled": scheduled if isinstance(scheduled, int) else 0,
}
```

The error path returns `{"pending": 0, "scheduled": 0}`. So the Python `/queue`
body has keys `{pending, scheduled}` and nothing else; likewise
`/status.queue`.

## Rust drift

`rust/crates/submate-server/src/lib.rs`:

- `QueueStats` serializes to `{pending, running, done, nodes}` (u64 each).
- `async fn queue` returns `Json<QueueStats>`, so `GET /queue` emits the
  four-key node-topology object.
- `async fn status` embeds `state.stats.stats()` under `"queue"`, so
  `/status.queue` is the same four-key object.

Net wire difference vs Python:

- `scheduled` is **dropped** (a Bazarr/monitoring client reading
  `body["scheduled"]` gets `null`/missing instead of an int).
- `running`, `done`, `nodes` are **added** keys Python never emits.

The divergence is deliberate at the architecture level — the Rust queue is a
SQLite-backed pull queue with node coordination, not Huey
(`rust/crates/submate-queue/src/lib.rs` module docs; `QueueStats` docstring in
the server crate explicitly says "**not** Huey's `pending` / `scheduled`").
The inline tests `ops_routes_queue_returns_zeroed_node_topology_stats` and
`ops_routes_queue_reflects_live_stats` pin the four-key shape on purpose, and
`rust/fixtures/server/core_router.json` sidesteps the inner queue shape by
only asserting `queue_key_present: true` for `/status`. So no parity gate
currently catches the mismatch.

## Decision needed

This is a real Python-contract break on a wire route, but it is grounded in an
intentional architecture change. One of:

1. **Accept the divergence (recommended) — but record it as a sanctioned
   contract delta.** Add a `compat` view: when serving `GET /queue` and
   `/status.queue`, emit Python-compatible keys so existing clients keep
   working — `{"pending": <queued count>, "scheduled": <scheduled/delayed
   count>}` — optionally alongside the richer node-topology fields. The Python
   `pending` maps to queued-and-due jobs; `scheduled` maps to jobs with a future
   `run_at` (the Rust queue has a `run_at` column, so a `schedule_size`
   equivalent — `COUNT(*) WHERE state='queued' AND run_at > now` — is
   expressible). This preserves the Bazarr/monitoring contract.

2. **Document the delta explicitly** in `rust/docs/architecture.md` and the
   server crate as an *approved* contract break (it is currently only described
   as an implementation note, not flagged as a deliberate wire-contract
   departure from the SPEC), and update CLAUDE.md's route table so downstream
   ports don't assume Python's `/queue` shape.

Whichever path, the `/queue` shape should stop being silently uncaptured: a
golden fixture should pin whatever shape is chosen, captured from the SPEC if
option 1, or annotated as an intentional delta if option 2.

## Falsifier

Capture the Python `/queue` body and diff its key set against the Rust route.

Python (the SPEC), shows the two-key shape:

```python
# from the repo root, with submate importable
from submate.queue.task_queue import TaskQueue  # construct as the server does
# or simply inspect the literal in TaskQueue.stats:
#   keys are exactly {"pending", "scheduled"}
```

Rust gate (currently RED against the Python contract — it asserts the four-key
node-topology shape):

```rust
// rust/crates/submate-server/src/lib.rs ops_routes_queue_returns_zeroed_node_topology_stats
let (status, body) = get_json(app(AppState::default()), "/queue").await;
let keys: std::collections::BTreeSet<_> =
    body.as_object().unwrap().keys().cloned().collect();
// Python SPEC says this set must equal {"pending", "scheduled"}:
assert_eq!(
    keys,
    ["pending", "scheduled"].iter().map(|s| s.to_string()).collect()
);
// Rust currently produces {"pending","running","done","nodes"} -> FAILS.
```

Concretely: `body["scheduled"]` is `null` in Rust but an integer in Python, and
`body["running"]`/`body["done"]`/`body["nodes"]` exist in Rust but are absent in
Python. Either the Rust route gains the Python-compatible keys (option 1) or the
SPEC table is amended to bless the new shape (option 2); today neither is true,
so the wire contract is unaligned and ungated.
