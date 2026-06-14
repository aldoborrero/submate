//! Central durable job store for the submate server.
//!
//! This is the coordination substrate the whole server↔node system runs on
//! (see `docs/architecture.md`). It is a **server-local** SQLite-backed
//! queue — nodes pull work over HTTP and never open this file. It is **not** a
//! Huey clone and **not** apalis; it is a hand-rolled `jobs` table with an
//! atomic claim, exponential-backoff retry, and startup lease reclaim.
//!
//! ## Schema
//!
//! A single `jobs` table:
//!
//! | column        | meaning                                                |
//! |---------------|--------------------------------------------------------|
//! | `id`          | autoincrement primary key                              |
//! | `kind`        | job kind (e.g. `transcribe`/`translate`) — routing key |
//! | `payload`     | opaque blob the caller round-trips (JSON, etc.)        |
//! | `state`       | `queued` \| `running` \| `done` \| `failed`            |
//! | `priority`    | higher is claimed first (Bazarr ASR > library scans)   |
//! | `requires_gpu`| `1` if the job may only run on a GPU node              |
//! | `attempts`    | how many times this job has been claimed               |
//! | `max_attempts`| attempts allowed before `fail` makes it terminal       |
//! | `run_at`      | earliest unix-epoch ms the job may be claimed          |
//! | `locked_by`   | worker/node id that holds the current lease            |
//! | `locked_at`   | unix-epoch ms the lease was taken (for reclaim)        |
//!
//! ## Capability routing
//!
//! Nodes are heterogeneous (GPU box, CPU box, translation-only). [`claim_with`]
//! takes the claiming node's [`NodeCapabilities`] and folds them into the claim
//! subquery so a node is only ever handed a job it can actually run: a
//! `requires_gpu` job is invisible to a CPU node, and a job whose `kind` is not
//! in the node's advertised set is skipped over rather than claimed-then-released.
//! Among the jobs a node *can* run, higher `priority` wins (then oldest
//! `run_at`), so the synchronous Bazarr-ASR path jumps ahead of bulk library
//! scans. Plain [`claim`] is the unrestricted form (any kind, GPU or not) used
//! by single-class workers and tests.
//!
//! ## Concurrency
//!
//! The connection runs in WAL mode with a `busy_timeout`, and [`claim`] is a
//! single atomic `UPDATE … WHERE id = (SELECT … LIMIT 1) RETURNING *`. SQLite
//! serialises writers, so two concurrent claimers can never receive the same
//! row: the `SELECT` subquery and the `UPDATE` happen inside one statement, and
//! the row leaves the `queued` state before the next writer's subquery runs.
//!
//! [`claim_with`]: JobStore::claim_with
//! [`claim`]: JobStore::claim

use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, OptionalExtension, Row};

pub mod models;
pub use models::{OutputFormat, SkipReason};

/// Errors surfaced by the job store.
#[derive(Debug, thiserror::Error)]
pub enum QueueError {
    /// An underlying SQLite operation failed.
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    /// An operation referenced a job id that does not exist.
    #[error("job {0} not found")]
    NotFound(JobId),
}

/// Result alias for store operations.
pub type Result<T> = std::result::Result<T, QueueError>;

/// Server-side job identifier (the `jobs.id` primary key).
pub type JobId = i64;

/// The full ordered column list, shared by every `SELECT`/`RETURNING` that
/// hydrates a [`Job`] so the projection and [`Job::from_row`] stay in lockstep.
const JOB_COLUMNS: &str = "id, kind, payload, state, priority, requires_gpu, \
     attempts, max_attempts, run_at, locked_by, locked_at";

/// Lifecycle state of a job.
///
/// Stored as its lowercase string form so the table is readable and stable
/// across versions (an integer discriminant would be brittle).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobState {
    /// Waiting to be claimed (and `run_at <= now`).
    Queued,
    /// Claimed by a worker; holds a lease (`locked_by` / `locked_at`).
    Running,
    /// Completed successfully — terminal.
    Done,
    /// Exhausted its attempts — terminal.
    Failed,
}

impl JobState {
    /// The textual form persisted in the `state` column.
    fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Done => "done",
            Self::Failed => "failed",
        }
    }

    fn from_str(s: &str) -> Self {
        match s {
            "queued" => Self::Queued,
            "running" => Self::Running,
            "done" => Self::Done,
            "failed" => Self::Failed,
            // The column is only ever written by this module, so an unknown
            // value means the DB was tampered with; treating it as `failed` is
            // the safe, non-claimable fallback.
            _ => Self::Failed,
        }
    }
}

/// A row of the `jobs` table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Job {
    /// Primary key.
    pub id: JobId,
    /// Job kind — the capability/routing key matched against a node's
    /// advertised task set in [`claim_with`](JobStore::claim_with).
    pub kind: String,
    /// Opaque caller payload.
    pub payload: String,
    /// Current lifecycle state.
    pub state: JobState,
    /// Claim ordering weight; higher is claimed first (default `0`).
    pub priority: i64,
    /// Whether the job may only run on a GPU-capable node.
    pub requires_gpu: bool,
    /// Number of times the job has been claimed.
    pub attempts: u32,
    /// Attempts permitted before `fail` becomes terminal.
    pub max_attempts: u32,
    /// Earliest unix-epoch ms the job may be claimed.
    pub run_at: i64,
    /// Worker that currently holds the lease, if `Running`.
    pub locked_by: Option<String>,
    /// Unix-epoch ms the current lease was taken, if `Running`.
    pub locked_at: Option<i64>,
}

impl Job {
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        let state: String = row.get("state")?;
        Ok(Self {
            id: row.get("id")?,
            kind: row.get("kind")?,
            payload: row.get("payload")?,
            state: JobState::from_str(&state),
            priority: row.get("priority")?,
            requires_gpu: row.get("requires_gpu")?,
            attempts: row.get("attempts")?,
            max_attempts: row.get("max_attempts")?,
            run_at: row.get("run_at")?,
            locked_by: row.get("locked_by")?,
            locked_at: row.get("locked_at")?,
        })
    }
}

/// Parameters for enqueuing a new job.
#[derive(Debug, Clone)]
pub struct NewJob {
    /// Job kind.
    pub kind: String,
    /// Opaque caller payload.
    pub payload: String,
    /// Claim ordering weight; higher is claimed first. Defaults to `0`, so a
    /// Bazarr-ASR job enqueued with a positive priority jumps ahead of bulk
    /// library scans left at the default.
    pub priority: i64,
    /// Whether the job may only run on a GPU-capable node.
    pub requires_gpu: bool,
    /// Attempts permitted before the job is failed for good.
    pub max_attempts: u32,
    /// Earliest unix-epoch ms the job may run (use `0` for "as soon as
    /// possible"; the store treats any past time as immediately eligible).
    pub run_at: i64,
}

impl NewJob {
    /// A job runnable immediately with a single attempt, default priority, and
    /// no GPU requirement.
    pub fn now(kind: impl Into<String>, payload: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            payload: payload.into(),
            priority: 0,
            requires_gpu: false,
            max_attempts: 1,
            run_at: 0,
        }
    }

    /// Set the attempt budget.
    pub fn with_max_attempts(mut self, max_attempts: u32) -> Self {
        self.max_attempts = max_attempts;
        self
    }

    /// Set the claim priority (higher is claimed first).
    pub fn with_priority(mut self, priority: i64) -> Self {
        self.priority = priority;
        self
    }

    /// Require the job to run on a GPU-capable node.
    pub fn requiring_gpu(mut self) -> Self {
        self.requires_gpu = true;
        self
    }
}

/// What a claiming node can run, folded into the [`claim_with`] query so a node
/// is never handed work it cannot execute.
///
/// Capabilities are advertised by the node at registration (`{gpu, tasks}` in
/// the node-coordination API) and surface here as the routing filter:
///
/// - `gpu` — the node has a GPU. A job with `requires_gpu = true` is only
///   visible to a node with `gpu = true`.
/// - `kinds` — the job kinds the node will accept (e.g. a translation-only node
///   that holds LLM credentials advertises just `["translate"]`). `None` means
///   "accept any kind"; an empty slice means "accept none".
///
/// [`claim_with`]: JobStore::claim_with
#[derive(Debug, Clone, Default)]
pub struct NodeCapabilities {
    /// Whether the node can run GPU-only jobs.
    pub gpu: bool,
    /// The job kinds the node accepts, or `None` to accept any kind.
    pub kinds: Option<Vec<String>>,
}

impl NodeCapabilities {
    /// A node that accepts any kind and has no GPU (the common CPU node).
    pub fn cpu() -> Self {
        Self {
            gpu: false,
            kinds: None,
        }
    }

    /// A node that accepts any kind and has a GPU.
    pub fn gpu() -> Self {
        Self {
            gpu: true,
            kinds: None,
        }
    }

    /// Restrict the node to the given job kinds.
    pub fn with_kinds(mut self, kinds: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.kinds = Some(kinds.into_iter().map(Into::into).collect());
        self
    }
}

/// Wall-clock source, abstracted so backoff and lease reclaim are testable.
///
/// Production code uses [`SystemClock`]; tests inject a controllable clock to
/// advance time deterministically without sleeping.
pub trait Clock: Send + Sync {
    /// Current time in unix-epoch milliseconds.
    fn now_ms(&self) -> i64;
}

/// A [`Clock`] backed by the system wall clock.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_ms(&self) -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as i64)
    }
}

/// Tuning knobs for retry backoff and lease reclaim.
#[derive(Debug, Clone, Copy)]
pub struct StoreConfig {
    /// Base delay (ms) for the first retry; doubles each attempt.
    pub backoff_base_ms: i64,
    /// Upper bound (ms) on a single retry's computed backoff.
    pub backoff_max_ms: i64,
    /// How long (ms) a lease is honoured before a running row is considered
    /// stale and eligible for reclaim on startup / sweep.
    pub lease_ms: i64,
    /// `busy_timeout` (ms) applied to the SQLite connection.
    pub busy_timeout_ms: u32,
}

impl Default for StoreConfig {
    fn default() -> Self {
        Self {
            backoff_base_ms: 1_000,
            backoff_max_ms: 5 * 60 * 1_000,
            lease_ms: 5 * 60 * 1_000,
            busy_timeout_ms: 5_000,
        }
    }
}

/// The durable job store.
///
/// Wraps one `rusqlite::Connection`. The store is not internally synchronised;
/// share it across threads by giving each thread its own [`JobStore`] over the
/// same database file (SQLite handles cross-connection locking), or guard a
/// single instance behind a `Mutex`. Atomicity of [`claim`] does not depend on
/// external locking — it is enforced by SQLite within the single statement.
///
/// [`claim`]: JobStore::claim
pub struct JobStore {
    conn: Connection,
    config: StoreConfig,
    clock: Box<dyn Clock>,
}

impl JobStore {
    /// Open (or create) a store at `path`, applying WAL + `busy_timeout` and
    /// creating the schema if absent. Uses the system clock and default config.
    pub fn open(path: impl AsRef<std::path::Path>) -> Result<Self> {
        let conn = Connection::open(path)?;
        Self::from_conn(conn, StoreConfig::default(), Box::new(SystemClock))
    }

    /// Open an in-memory store (primarily for tests).
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        Self::from_conn(conn, StoreConfig::default(), Box::new(SystemClock))
    }

    /// Open an in-memory store with an explicit [`StoreConfig`] and [`Clock`].
    ///
    /// This is the seam for deterministic time control without exposing the
    /// underlying `rusqlite::Connection`: callers (including downstream crates'
    /// tests) inject a controllable clock to exercise lease reclaim and backoff
    /// without sleeping.
    pub fn in_memory_with(config: StoreConfig, clock: Box<dyn Clock>) -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        Self::from_conn(conn, config, clock)
    }

    /// Construct a store from an existing connection with explicit config and
    /// clock. Applies pragmas and ensures the schema exists.
    pub fn from_conn(conn: Connection, config: StoreConfig, clock: Box<dyn Clock>) -> Result<Self> {
        // WAL gives concurrent readers alongside a writer; busy_timeout lets a
        // writer wait out a contending writer instead of erroring with
        // SQLITE_BUSY. `journal_mode` is a no-op (returns the new mode) on
        // in-memory databases, which is fine.
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.busy_timeout(std::time::Duration::from_millis(
            config.busy_timeout_ms as u64,
        ))?;

        let store = Self {
            conn,
            config,
            clock,
        };
        store.init_schema()?;
        Ok(store)
    }

    fn init_schema(&self) -> Result<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS jobs (
                id           INTEGER PRIMARY KEY AUTOINCREMENT,
                kind         TEXT    NOT NULL,
                payload      TEXT    NOT NULL,
                state        TEXT    NOT NULL DEFAULT 'queued',
                priority     INTEGER NOT NULL DEFAULT 0,
                requires_gpu INTEGER NOT NULL DEFAULT 0,
                attempts     INTEGER NOT NULL DEFAULT 0,
                max_attempts INTEGER NOT NULL DEFAULT 1,
                run_at       INTEGER NOT NULL DEFAULT 0,
                locked_by    TEXT,
                locked_at    INTEGER
            );
            -- The claim subquery filters on (state, run_at) and orders by
            -- (priority, run_at); this composite index keeps that the hot path
            -- as the table grows. priority is DESC because higher wins.
            CREATE INDEX IF NOT EXISTS idx_jobs_claimable
                ON jobs (state, priority DESC, run_at, id);",
        )?;
        Ok(())
    }

    /// Insert a new `queued` job, returning its id.
    pub fn enqueue(&self, job: &NewJob) -> Result<JobId> {
        self.conn.execute(
            "INSERT INTO jobs
                (kind, payload, state, priority, requires_gpu, attempts, max_attempts, run_at)
             VALUES (?1, ?2, 'queued', ?3, ?4, 0, ?5, ?6)",
            rusqlite::params![
                job.kind,
                job.payload,
                job.priority,
                job.requires_gpu,
                job.max_attempts,
                job.run_at,
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Atomically claim the next eligible job for `worker`, with no capability
    /// restriction (any kind, GPU-required or not).
    ///
    /// This is the unrestricted form for single-class workers and tests; a
    /// heterogeneous deployment uses [`claim_with`](JobStore::claim_with).
    /// Selects the single highest-priority, then oldest (`ORDER BY priority
    /// DESC, run_at`) `queued` row whose `run_at <= now`, flips it to `running`,
    /// records the lease, bumps `attempts`, and returns the resulting row — all
    /// in one statement so concurrent callers can never claim the same job.
    /// Returns `Ok(None)` when nothing is eligible.
    pub fn claim(&self, worker: &str) -> Result<Option<Job>> {
        // `cpu()` accepts any kind; the GPU filter is unrestricted here because
        // the unrestricted claim must still be able to take GPU-required jobs
        // (single-class GPU workers / tests rely on it).
        self.claim_capable(worker, &NodeCapabilities::cpu(), false)
    }

    /// Atomically claim the next eligible job the node described by `caps` can
    /// actually run.
    ///
    /// Extends [`claim`](JobStore::claim) with capability routing: a job with
    /// `requires_gpu` is only claimable when `caps.gpu` is true, and a job whose
    /// `kind` is not in `caps.kinds` is skipped (jobs are filtered inside the
    /// claim subquery, so a non-matching job is never claimed-then-released —
    /// it simply stays `queued` for a capable node). Among the runnable jobs the
    /// highest `priority` is taken first (then oldest `run_at`), so a
    /// high-priority Bazarr-ASR job is claimed ahead of bulk library scans.
    ///
    /// Atomicity is identical to [`claim`](JobStore::claim): the filter lives in
    /// the `SELECT … LIMIT 1` subquery of the single claiming `UPDATE`.
    pub fn claim_with(&self, worker: &str, caps: &NodeCapabilities) -> Result<Option<Job>> {
        self.claim_capable(worker, caps, true)
    }

    /// Shared claim implementation. `enforce_gpu` gates whether `requires_gpu`
    /// jobs are filtered by `caps.gpu` ([`claim`](JobStore::claim) leaves them
    /// claimable; [`claim_with`](JobStore::claim_with) enforces the match).
    fn claim_capable(
        &self,
        worker: &str,
        caps: &NodeCapabilities,
        enforce_gpu: bool,
    ) -> Result<Option<Job>> {
        let now = self.clock.now_ms();

        // Build the capability predicate and its bound parameters. `worker` and
        // `now` are always ?1/?2; capability values follow as ?3.. .
        let mut params: Vec<Box<dyn rusqlite::ToSql>> =
            vec![Box::new(worker.to_owned()), Box::new(now)];
        let mut predicate = String::new();

        if enforce_gpu && !caps.gpu {
            // A non-GPU node can never take a GPU-required job.
            predicate.push_str(" AND requires_gpu = 0");
        }

        if let Some(kinds) = &caps.kinds {
            if kinds.is_empty() {
                // A node that accepts no kinds can claim nothing.
                return Ok(None);
            }
            let start = params.len() + 1;
            let placeholders: Vec<String> = (start..start + kinds.len())
                .map(|i| format!("?{i}"))
                .collect();
            predicate.push_str(&format!(" AND kind IN ({})", placeholders.join(", ")));
            for kind in kinds {
                params.push(Box::new(kind.clone()));
            }
        }

        let sql = format!(
            "UPDATE jobs
                SET state = 'running',
                    locked_by = ?1,
                    locked_at = ?2,
                    attempts = attempts + 1
              WHERE id = (
                  SELECT id FROM jobs
                   WHERE state = 'queued' AND run_at <= ?2{predicate}
                   ORDER BY priority DESC, run_at, id
                   LIMIT 1
              )
              RETURNING {JOB_COLUMNS}"
        );

        let param_refs: Vec<&dyn rusqlite::ToSql> =
            params.iter().map(std::convert::AsRef::as_ref).collect();
        let job = self
            .conn
            .query_row(&sql, param_refs.as_slice(), Job::from_row)
            .optional()?;
        Ok(job)
    }

    /// Mark a `running` job as `done` and release its lease.
    pub fn complete(&self, id: JobId) -> Result<()> {
        let changed = self.conn.execute(
            "UPDATE jobs
                SET state = 'done', locked_by = NULL, locked_at = NULL
              WHERE id = ?1",
            rusqlite::params![id],
        )?;
        if changed == 0 {
            return Err(QueueError::NotFound(id));
        }
        Ok(())
    }

    /// Release a `running` job back to `queued` without consuming an attempt or
    /// applying backoff — the inverse of [`claim`](JobStore::claim).
    ///
    /// Unlike [`fail`](JobStore::fail), this is for "claimed but cannot run
    /// here" (e.g. a capability mismatch): the job is immediately re-claimable by
    /// another worker and its attempt budget is untouched. Returns
    /// [`QueueError::NotFound`] if `id` does not exist.
    pub fn release(&self, id: JobId) -> Result<()> {
        let changed = self.conn.execute(
            "UPDATE jobs
                SET state = 'queued', run_at = 0,
                    locked_by = NULL, locked_at = NULL,
                    attempts = MAX(attempts - 1, 0)
              WHERE id = ?1",
            rusqlite::params![id],
        )?;
        if changed == 0 {
            return Err(QueueError::NotFound(id));
        }
        Ok(())
    }

    /// Report a failed attempt.
    ///
    /// If the job still has attempts left it is re-`queued` with an
    /// exponential-backoff `run_at` (`base * 2^(attempts-1)`, capped at
    /// `backoff_max_ms`) and its lease cleared. Once attempts are exhausted it
    /// becomes terminally `failed`. Returns the post-transition [`Job`].
    pub fn fail(&self, id: JobId) -> Result<Job> {
        let now = self.clock.now_ms();
        let job = self.get(id)?.ok_or(QueueError::NotFound(id))?;

        if job.attempts >= job.max_attempts {
            self.conn.execute(
                "UPDATE jobs
                    SET state = 'failed', locked_by = NULL, locked_at = NULL
                  WHERE id = ?1",
                rusqlite::params![id],
            )?;
        } else {
            let run_at = now + self.backoff_ms(job.attempts);
            self.conn.execute(
                "UPDATE jobs
                    SET state = 'queued', run_at = ?2,
                        locked_by = NULL, locked_at = NULL
                  WHERE id = ?1",
                rusqlite::params![id, run_at],
            )?;
        }

        self.get(id)?.ok_or(QueueError::NotFound(id))
    }

    /// Extend the lease on every `running` job currently held by `worker`,
    /// re-stamping `locked_at` to *now* so a fresh lease window starts.
    ///
    /// This is the server side of a node heartbeat: a live node periodically
    /// touches its leases so [`reclaim_stale_leases`] does not steal jobs that
    /// are still being worked. Jobs held by other workers are untouched.
    /// Returns the number of leases refreshed.
    ///
    /// [`reclaim_stale_leases`]: JobStore::reclaim_stale_leases
    pub fn touch_leases(&self, worker: &str) -> Result<usize> {
        let now = self.clock.now_ms();
        let changed = self.conn.execute(
            "UPDATE jobs
                SET locked_at = ?2
              WHERE state = 'running' AND locked_by = ?1",
            rusqlite::params![worker, now],
        )?;
        Ok(changed)
    }

    /// Reclaim stale leases: any `running` row whose lease expired
    /// (`locked_at + lease_ms < now`) returns to `queued` for re-claim. Called
    /// on startup (crash recovery) and may be run periodically as a sweep.
    /// Returns the number of jobs reclaimed.
    pub fn reclaim_stale_leases(&self) -> Result<usize> {
        let now = self.clock.now_ms();
        let deadline = now - self.config.lease_ms;
        let changed = self.conn.execute(
            "UPDATE jobs
                SET state = 'queued', locked_by = NULL, locked_at = NULL
              WHERE state = 'running' AND locked_at <= ?1",
            rusqlite::params![deadline],
        )?;
        Ok(changed)
    }

    /// Fetch a job by id.
    pub fn get(&self, id: JobId) -> Result<Option<Job>> {
        let job = self
            .conn
            .query_row(
                &format!("SELECT {JOB_COLUMNS} FROM jobs WHERE id = ?1"),
                rusqlite::params![id],
                Job::from_row,
            )
            .optional()?;
        Ok(job)
    }

    /// Count jobs currently in `state` (mostly for tests/metrics).
    pub fn count(&self, state: JobState) -> Result<i64> {
        let n = self.conn.query_row(
            "SELECT COUNT(*) FROM jobs WHERE state = ?1",
            rusqlite::params![state.as_str()],
            |row| row.get(0),
        )?;
        Ok(n)
    }

    /// Exponential backoff for the `attempts`-th failure (1-based), capped.
    fn backoff_ms(&self, attempts: u32) -> i64 {
        // attempts is the post-increment count, so the first failure is 1.
        let shift = attempts.saturating_sub(1).min(30);
        let delay = self.config.backoff_base_ms.saturating_mul(1_i64 << shift);
        delay.min(self.config.backoff_max_ms)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicI64, Ordering};
    use std::sync::{Arc, Barrier};

    /// A controllable clock for deterministic backoff / lease tests.
    #[derive(Clone, Default)]
    struct TestClock(Arc<AtomicI64>);

    impl TestClock {
        fn new(ms: i64) -> Self {
            Self(Arc::new(AtomicI64::new(ms)))
        }
        fn set(&self, ms: i64) {
            self.0.store(ms, Ordering::SeqCst);
        }
        fn advance(&self, ms: i64) {
            self.0.fetch_add(ms, Ordering::SeqCst);
        }
    }

    impl Clock for TestClock {
        fn now_ms(&self) -> i64 {
            self.0.load(Ordering::SeqCst)
        }
    }

    fn store_with(clock: impl Clock + 'static, config: StoreConfig) -> JobStore {
        JobStore::from_conn(
            Connection::open_in_memory().unwrap(),
            config,
            Box::new(clock),
        )
        .unwrap()
    }

    #[test]
    fn lifecycle_queued_running_done() {
        let store = JobStore::open_in_memory().unwrap();
        let id = store.enqueue(&NewJob::now("transcribe", "{}")).unwrap();

        assert_eq!(store.get(id).unwrap().unwrap().state, JobState::Queued);
        assert_eq!(store.count(JobState::Queued).unwrap(), 1);

        let claimed = store.claim("w1").unwrap().expect("a job to claim");
        assert_eq!(claimed.id, id);
        assert_eq!(claimed.state, JobState::Running);
        assert_eq!(claimed.attempts, 1);
        assert_eq!(claimed.locked_by.as_deref(), Some("w1"));
        assert!(claimed.locked_at.is_some());

        // Nothing else to claim now.
        assert!(store.claim("w2").unwrap().is_none());

        store.complete(id).unwrap();
        let done = store.get(id).unwrap().unwrap();
        assert_eq!(done.state, JobState::Done);
        assert!(done.locked_by.is_none());
        assert!(done.locked_at.is_none());
        assert_eq!(store.count(JobState::Done).unwrap(), 1);
    }

    /// Falsifier core: N threads claiming concurrently get distinct jobs, with
    /// no double-claim and no job left behind.
    #[test]
    fn claim_atomic_concurrent_distinct() {
        // A shared file-backed DB so every thread's connection contends on the
        // same SQLite database (in-memory DBs are per-connection).
        let dir = std::env::temp_dir().join(format!("submate-queue-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("claim_atomic.sqlite");
        let _ = std::fs::remove_file(&path);

        let n: usize = 32;
        let writer = JobStore::open(&path).unwrap();
        for i in 0..n {
            writer
                .enqueue(&NewJob::now("transcribe", format!("job-{i}")))
                .unwrap();
        }

        let barrier = Arc::new(Barrier::new(n));
        let path = Arc::new(path);
        let mut handles = Vec::new();
        for t in 0..n {
            let barrier = Arc::clone(&barrier);
            let path = Arc::clone(&path);
            handles.push(std::thread::spawn(move || {
                let store = JobStore::open(path.as_ref()).unwrap();
                let worker = format!("w{t}");
                barrier.wait();
                let mut claimed = Vec::new();
                // Each thread keeps claiming until the queue drains; the union
                // of claims must be exactly the N jobs, each once.
                while let Some(job) = store.claim(&worker).unwrap() {
                    claimed.push(job.id);
                }
                claimed
            }));
        }

        let mut all: Vec<JobId> = handles
            .into_iter()
            .flat_map(|h| h.join().unwrap())
            .collect();
        all.sort_unstable();
        let before = all.len();
        all.dedup();
        assert_eq!(all.len(), before, "a job was double-claimed");
        assert_eq!(all.len(), n, "not every job was claimed exactly once");

        let _ = std::fs::remove_file(path.as_ref());
    }

    #[test]
    fn failed_requeues_with_backoff_then_terminal() {
        let clock = TestClock::new(0);
        let config = StoreConfig {
            backoff_base_ms: 1_000,
            backoff_max_ms: 60_000,
            ..StoreConfig::default()
        };
        let store = store_with(clock.clone(), config);

        let id = store
            .enqueue(&NewJob::now("transcribe", "{}").with_max_attempts(3))
            .unwrap();

        // First claim + fail -> re-queued with base backoff (1s).
        let job = store.claim("w").unwrap().unwrap();
        assert_eq!(job.attempts, 1);
        let after = store.fail(id).unwrap();
        assert_eq!(after.state, JobState::Queued);
        assert_eq!(after.run_at, 1_000, "first retry uses base backoff");
        assert!(after.locked_by.is_none());

        // Not yet eligible at t=0.
        assert!(store.claim("w").unwrap().is_none());
        // Eligible once the clock reaches run_at.
        clock.set(1_000);
        let job = store.claim("w").unwrap().unwrap();
        assert_eq!(job.attempts, 2);

        // Second fail -> backoff doubles (base * 2^1 = 2s past now=1000).
        let after = store.fail(id).unwrap();
        assert_eq!(after.state, JobState::Queued);
        assert_eq!(after.run_at, 1_000 + 2_000);

        clock.set(after.run_at);
        let job = store.claim("w").unwrap().unwrap();
        assert_eq!(job.attempts, 3, "attempts exhausted on this claim");

        // Third fail -> attempts == max_attempts -> terminal failed.
        let after = store.fail(id).unwrap();
        assert_eq!(after.state, JobState::Failed);
        assert!(after.locked_by.is_none());
        assert!(after.locked_at.is_none());
        assert_eq!(store.count(JobState::Failed).unwrap(), 1);
    }

    #[test]
    fn backoff_is_capped() {
        let clock = TestClock::new(0);
        let config = StoreConfig {
            backoff_base_ms: 1_000,
            backoff_max_ms: 4_000,
            ..StoreConfig::default()
        };
        let store = store_with(clock.clone(), config);
        let id = store
            .enqueue(&NewJob::now("k", "p").with_max_attempts(100))
            .unwrap();

        // Drive several failures; backoff must never exceed the cap.
        for _ in 0..6 {
            let now = clock.now_ms();
            // Make the job eligible regardless of accumulated run_at.
            let job = {
                // fast-forward to whatever run_at the job currently sits at
                let j = store.get(id).unwrap().unwrap();
                clock.set(j.run_at.max(now));
                store.claim("w").unwrap().unwrap()
            };
            assert_eq!(job.state, JobState::Running);
            let after = store.fail(id).unwrap();
            let delay = after.run_at - clock.now_ms();
            assert!(delay <= 4_000, "backoff {delay} exceeded cap");
        }
    }

    #[test]
    fn stale_lease_is_reclaimed() {
        let clock = TestClock::new(10_000);
        let config = StoreConfig {
            lease_ms: 5_000,
            ..StoreConfig::default()
        };
        let store = store_with(clock.clone(), config);

        let id = store.enqueue(&NewJob::now("transcribe", "{}")).unwrap();
        let job = store.claim("dead-worker").unwrap().unwrap();
        assert_eq!(job.state, JobState::Running);
        assert_eq!(job.locked_at, Some(10_000));

        // Within the lease window: nothing reclaimed.
        clock.set(10_000 + 4_000);
        assert_eq!(store.reclaim_stale_leases().unwrap(), 0);
        assert_eq!(store.get(id).unwrap().unwrap().state, JobState::Running);

        // Past the lease window: the row returns to queued, lease cleared.
        clock.advance(2_000); // now = 16_000 > 10_000 + 5_000
        assert_eq!(store.reclaim_stale_leases().unwrap(), 1);
        let reclaimed = store.get(id).unwrap().unwrap();
        assert_eq!(reclaimed.state, JobState::Queued);
        assert!(reclaimed.locked_by.is_none());
        assert!(reclaimed.locked_at.is_none());

        // And it is claimable again (by a live worker).
        let again = store.claim("live-worker").unwrap().unwrap();
        assert_eq!(again.id, id);
        assert_eq!(again.attempts, 2, "reclaim preserves the attempt count");
    }

    #[test]
    fn run_at_in_the_future_is_not_claimable_yet() {
        let clock = TestClock::new(0);
        let store = store_with(clock.clone(), StoreConfig::default());
        let id = store
            .enqueue(&NewJob {
                kind: "delayed".into(),
                payload: "{}".into(),
                priority: 0,
                requires_gpu: false,
                max_attempts: 1,
                run_at: 5_000,
            })
            .unwrap();

        assert!(store.claim("w").unwrap().is_none());
        clock.set(5_000);
        assert_eq!(store.claim("w").unwrap().unwrap().id, id);
    }

    #[test]
    fn complete_and_fail_unknown_job_error() {
        let store = JobStore::open_in_memory().unwrap();
        assert!(matches!(
            store.complete(999),
            Err(QueueError::NotFound(999))
        ));
        assert!(matches!(store.fail(999), Err(QueueError::NotFound(999))));
    }

    /// Falsifier (`claim_routing`): a GPU-only job is never handed to a CPU
    /// node, but a GPU node may take it.
    #[test]
    fn claim_routing_gpu_job_skips_cpu_node() {
        let store = JobStore::open_in_memory().unwrap();
        let gpu_id = store
            .enqueue(&NewJob::now("transcribe", "{}").requiring_gpu())
            .unwrap();

        // The CPU node sees no work: the GPU-only job is filtered out, not
        // claimed-then-released, so it stays queued.
        assert!(
            store
                .claim_with("cpu", &NodeCapabilities::cpu())
                .unwrap()
                .is_none()
        );
        assert_eq!(store.get(gpu_id).unwrap().unwrap().state, JobState::Queued);
        assert_eq!(store.count(JobState::Queued).unwrap(), 1);

        // A GPU node claims it.
        let job = store
            .claim_with("gpu", &NodeCapabilities::gpu())
            .unwrap()
            .expect("gpu node claims the gpu job");
        assert_eq!(job.id, gpu_id);
        assert!(job.requires_gpu);
        assert_eq!(job.locked_by.as_deref(), Some("gpu"));
    }

    /// Falsifier (`claim_routing`): given two eligible jobs, the higher-priority
    /// one is claimed first regardless of enqueue order.
    #[test]
    fn claim_routing_higher_priority_first() {
        let store = JobStore::open_in_memory().unwrap();
        // Enqueue the low-priority scan first so plain FIFO would pick it.
        let scan = store
            .enqueue(&NewJob::now("transcribe", "scan").with_priority(0))
            .unwrap();
        let asr = store
            .enqueue(&NewJob::now("transcribe", "asr").with_priority(10))
            .unwrap();

        let first = store.claim("w").unwrap().expect("a job to claim");
        assert_eq!(first.id, asr, "the higher-priority job is claimed first");

        let second = store.claim("w").unwrap().expect("the remaining job");
        assert_eq!(second.id, scan);
    }

    /// A node restricted to specific kinds only claims those kinds; a
    /// non-matching job is left queued for a capable node.
    #[test]
    fn claim_routing_kind_filter() {
        let store = JobStore::open_in_memory().unwrap();
        let transcribe = store.enqueue(&NewJob::now("transcribe", "{}")).unwrap();
        let translate = store.enqueue(&NewJob::now("translate", "{}")).unwrap();

        // A translation-only node (holds LLM creds) skips the transcribe job.
        let caps = NodeCapabilities::cpu().with_kinds(["translate"]);
        let job = store.claim_with("llm", &caps).unwrap().unwrap();
        assert_eq!(job.id, translate);

        // Nothing else matches for this node; the transcribe job is still queued.
        assert!(store.claim_with("llm", &caps).unwrap().is_none());
        assert_eq!(
            store.get(transcribe).unwrap().unwrap().state,
            JobState::Queued
        );

        // An unrestricted node takes the leftover transcribe job.
        let job = store.claim("any").unwrap().unwrap();
        assert_eq!(job.id, transcribe);
    }

    /// A node that advertises no kinds claims nothing.
    #[test]
    fn claim_routing_empty_kinds_claims_nothing() {
        let store = JobStore::open_in_memory().unwrap();
        store.enqueue(&NewJob::now("transcribe", "{}")).unwrap();
        let caps = NodeCapabilities::cpu().with_kinds(Vec::<String>::new());
        assert!(store.claim_with("none", &caps).unwrap().is_none());
        assert_eq!(store.count(JobState::Queued).unwrap(), 1);
    }

    /// Priority and capability compose: a CPU node, faced with a high-priority
    /// GPU job and a lower-priority CPU job, takes the CPU job it can actually
    /// run rather than stalling on the higher-priority one it cannot.
    #[test]
    fn claim_routing_priority_respects_capability() {
        let store = JobStore::open_in_memory().unwrap();
        let _gpu = store
            .enqueue(
                &NewJob::now("transcribe", "gpu")
                    .with_priority(100)
                    .requiring_gpu(),
            )
            .unwrap();
        let cpu = store
            .enqueue(&NewJob::now("transcribe", "cpu").with_priority(1))
            .unwrap();

        let job = store
            .claim_with("cpu", &NodeCapabilities::cpu())
            .unwrap()
            .expect("cpu node takes the runnable job");
        assert_eq!(job.id, cpu);
    }
}
