//! Processing-node agent: pull work from the server, run the local Whisper/translation dispatcher, report results.
//!
//! The dispatcher is the per-node execution core. It holds a [`tokio::sync::Semaphore`]
//! sized to the node's runner count and gates every transcription behind a permit,
//! so at most `runners` clips transcribe concurrently — the rest wait for a permit
//! to free. This is the in-process concurrency cap that Python's queue worked around
//! with a separate worker process.
//!
//! The heavy CPU work runs on a blocking thread via [`tokio::task::spawn_blocking`],
//! keeping the async runtime responsive. The blocking step is injectable so tests can
//! drive concurrency with a barrier/counter without loading a model; the real wiring
//! (feature `model`) forwards to [`submate_whisper::transcribe_pcm`].
//!
//! # Agent pull-loop
//!
//! [`Agent`] is the `submate node --server <url>` worker. It registers its
//! capabilities, then long-polls the server for work:
//!
//! ```text
//! register(capabilities)
//! loop {
//!     request-work  ──204──▶ poll again
//!         │ Work
//!         ▼
//!     GET audio  →  Dispatcher  →  POST progress  →  POST result
//!     POST heartbeat
//! }
//! ```
//!
//! The audio-processing step is injectable through [`JobProcessor`]: the real
//! node decodes the server's PCM and runs it through the [`Dispatcher`] into the
//! Whisper/translation pipeline, while tests supply a closure that returns a
//! canned subtitle without loading a model. The HTTP transport is `reqwest`
//! speaking the `submate-proto` JSON contract.
//!
//! When the server is unreachable the loop reconnects with exponential backoff
//! plus jitter (the repo's network-retry convention), so a transient outage or a
//! restarting coordinator does not tear the node down — it keeps retrying until
//! the server comes back.

use std::future::Future;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Once};
use std::time::Duration;

use submate_proto::{
    Heartbeat, JobOpts, JobOutcome, JobResult, NodeRegister, NodeRegistered, Progress, WorkRequest,
    WorkResponse,
};
use submate_whisper::{WhisperError, WhisperResult};
use tokio::sync::Semaphore;

/// Caps concurrent transcriptions on a node to its runner count.
///
/// Clone is cheap: every clone shares the same underlying semaphore, so the
/// concurrency limit is enforced across all handles.
#[derive(Clone)]
pub struct Dispatcher {
    semaphore: Arc<Semaphore>,
    runners: usize,
}

impl Dispatcher {
    /// Build a dispatcher that allows `runners` transcriptions to run at once.
    ///
    /// # Panics
    ///
    /// Panics if `runners` is zero — a node with no runners can never make
    /// progress, so it is a configuration error rather than a runtime state.
    pub fn new(runners: usize) -> Self {
        assert!(runners > 0, "a node must have at least one runner");
        Self {
            semaphore: Arc::new(Semaphore::new(runners)),
            runners,
        }
    }

    /// The configured runner count (the concurrency ceiling).
    pub fn runners(&self) -> usize {
        self.runners
    }

    /// Permits currently available — i.e. how many more transcriptions could
    /// start right now without waiting.
    pub fn available_permits(&self) -> usize {
        self.semaphore.available_permits()
    }

    /// Run a blocking transcription step under a runner permit.
    ///
    /// Acquires a permit (waiting if all `runners` are busy), then runs `job`
    /// on a blocking thread via [`tokio::task::spawn_blocking`]. The permit is
    /// held for the entire duration of `job` and released when it returns, so
    /// the concurrency cap covers the actual work, not just the dispatch.
    ///
    /// `job` is the injectable blocking step: real callers pass a closure that
    /// invokes whisper.cpp inference; tests pass a closure that blocks on a
    /// barrier and bumps a counter to observe the cap.
    pub async fn transcribe_with<F>(&self, job: F) -> Result<WhisperResult, WhisperError>
    where
        F: FnOnce() -> Result<WhisperResult, WhisperError> + Send + 'static,
    {
        // Holding the owned permit alive until the blocking task finishes keeps
        // the slot reserved for the whole transcription.
        let permit = Arc::clone(&self.semaphore)
            .acquire_owned()
            .await
            .expect("dispatcher semaphore is never closed");

        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            job()
        })
        .await
        .map_err(|e| WhisperError::Join(e.to_string()))?
    }

    /// Transcribe a PCM clip through [`submate_whisper::transcribe_pcm`] under a
    /// runner permit.
    ///
    /// Available only with the `model` feature, which pulls in whisper.cpp. The
    /// permit is held across the whole inference call so concurrency stays
    /// capped at the runner count.
    #[cfg(feature = "model")]
    pub async fn transcribe_pcm(
        &self,
        model_path: impl Into<String>,
        pcm: Vec<f32>,
        options: submate_whisper::TranscribeOptions,
    ) -> Result<WhisperResult, WhisperError> {
        let model_path = model_path.into();
        let _permit = self
            .semaphore
            .acquire()
            .await
            .expect("dispatcher semaphore is never closed");
        submate_whisper::transcribe_pcm(model_path, pcm, options).await
    }
}

/// Errors the agent can surface while talking to the server.
///
/// Transport failures (`Http`) are the *retryable* class — the run loop treats
/// them as "server unavailable" and reconnects with backoff. A `Status` is the
/// server answering with an unexpected code (e.g. a `404` on register), which is
/// a contract/configuration problem rather than a transient outage.
#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    /// The HTTP request never completed (connection refused, timeout, DNS, …).
    #[error("transport error talking to server: {0}")]
    Http(#[from] reqwest::Error),
    /// The server responded with an unexpected status code.
    #[error("unexpected {status} from {endpoint}")]
    Status {
        /// The endpoint path that returned the unexpected code.
        endpoint: String,
        /// The HTTP status the server returned.
        status: reqwest::StatusCode,
    },
}

impl AgentError {
    /// Whether this error should trigger a reconnect-with-backoff rather than
    /// tearing the agent down: transport failures mean the server is (probably
    /// temporarily) unreachable.
    fn is_retryable(&self) -> bool {
        matches!(self, AgentError::Http(_))
    }
}

/// Turns a fetched audio payload into a terminal [`JobOutcome`].
///
/// This is the seam between the network loop and the local compute: the real
/// node decodes the server's PCM and runs it through the [`Dispatcher`] into the
/// Whisper/translation pipeline (see [`whisper_processor`]); tests inject a
/// closure that returns a canned subtitle so the pull-loop can be exercised
/// without loading a model.
///
/// The PCM is the raw bytes from `GET {audio_url}` (the server ships
/// s16le/mono/16k, or f32 — see `architecture.md`); `opts` carries the model,
/// language hints, and translation backend for the job.
pub trait JobProcessor: Send + Sync {
    /// Process one job's audio, yielding the subtitle output on success or a
    /// message on failure. Errors are reported back to the server as a failed
    /// [`JobResult`] rather than panicking the loop.
    fn process(
        &self,
        opts: &JobOpts,
        pcm: Vec<u8>,
    ) -> impl Future<Output = Result<String, String>> + Send;
}

impl<F, Fut> JobProcessor for F
where
    F: Fn(&JobOpts, Vec<u8>) -> Fut + Send + Sync,
    Fut: Future<Output = Result<String, String>> + Send,
{
    fn process(
        &self,
        opts: &JobOpts,
        pcm: Vec<u8>,
    ) -> impl Future<Output = Result<String, String>> + Send {
        self(opts, pcm)
    }
}

/// Tuning knobs for the agent's reconnect/backoff and poll pacing.
#[derive(Debug, Clone, Copy)]
pub struct AgentConfig {
    /// First reconnect delay after the server becomes unreachable; doubles each
    /// consecutive failure up to [`reconnect_max`](AgentConfig::reconnect_max).
    pub reconnect_base: Duration,
    /// Upper bound on a single reconnect delay (before jitter).
    pub reconnect_max: Duration,
    /// Delay between poll cycles after a `204 No Work`, so an idle node does not
    /// hot-loop the server when its `request-work` returns immediately.
    pub idle_poll_delay: Duration,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            reconnect_base: Duration::from_millis(500),
            reconnect_max: Duration::from_secs(30),
            idle_poll_delay: Duration::from_millis(500),
        }
    }
}

/// The `submate node --server <url>` agent: registers, then pulls and runs work.
///
/// Generic over the [`JobProcessor`] `P` so the same loop drives both the real
/// Whisper pipeline and a test stub. Construct with [`Agent::new`], then call
/// [`run`](Agent::run) for the lifetime loop or [`run_once`](Agent::run_once)
/// for a single register-then-poll cycle.
pub struct Agent<P, S = TokioSleeper> {
    http: reqwest::Client,
    /// Server base URL with no trailing slash (e.g. `http://server:9000`).
    base_url: String,
    register: NodeRegister,
    dispatcher: Dispatcher,
    processor: P,
    config: AgentConfig,
    sleeper: S,
}

/// How the agent waits between retries, abstracted so tests can run the backoff
/// loop without real time passing.
pub trait Sleeper: Send + Sync {
    /// Sleep for `dur`.
    fn sleep(&self, dur: Duration) -> impl Future<Output = ()> + Send;
}

/// Production [`Sleeper`] backed by [`tokio::time::sleep`].
#[derive(Debug, Clone, Copy, Default)]
pub struct TokioSleeper;

impl Sleeper for TokioSleeper {
    fn sleep(&self, dur: Duration) -> impl Future<Output = ()> + Send {
        tokio::time::sleep(dur)
    }
}

impl<P: JobProcessor> Agent<P> {
    /// Build an agent for `base_url` advertising `register`'s capabilities,
    /// running jobs through `dispatcher` + `processor` with default pacing.
    ///
    /// A trailing slash on `base_url` is stripped so endpoint paths join cleanly.
    pub fn new(base_url: impl Into<String>, register: NodeRegister, dispatcher: Dispatcher, processor: P) -> Self {
        Self::with_config(base_url, register, dispatcher, processor, AgentConfig::default())
    }

    /// Like [`new`](Agent::new) but with explicit backoff/poll [`AgentConfig`].
    pub fn with_config(
        base_url: impl Into<String>,
        register: NodeRegister,
        dispatcher: Dispatcher,
        processor: P,
        config: AgentConfig,
    ) -> Self {
        Agent {
            http: reqwest::Client::new(),
            base_url: base_url.into().trim_end_matches('/').to_string(),
            register,
            dispatcher,
            processor,
            config,
            sleeper: TokioSleeper,
        }
    }
}

impl<P: JobProcessor, S: Sleeper> Agent<P, S> {
    /// Swap in a custom [`Sleeper`] (tests inject one that records delays and
    /// returns immediately so the backoff loop runs without real waits).
    pub fn with_sleeper<S2: Sleeper>(self, sleeper: S2) -> Agent<P, S2> {
        Agent {
            http: self.http,
            base_url: self.base_url,
            register: self.register,
            dispatcher: self.dispatcher,
            processor: self.processor,
            config: self.config,
            sleeper,
        }
    }

    /// The dispatcher this agent runs jobs through (its runner-count cap).
    pub fn dispatcher(&self) -> &Dispatcher {
        &self.dispatcher
    }

    /// Register the node, then run the pull-loop forever.
    ///
    /// Registration and every poll cycle are wrapped in reconnect-with-backoff:
    /// while the server is unreachable the agent keeps retrying (exponential
    /// backoff + jitter), so a coordinator restart or a network blip never ends
    /// the agent. Non-transport errors (an unexpected status) propagate, since
    /// they signal a misconfiguration retrying cannot fix.
    pub async fn run(&self) -> Result<(), AgentError> {
        // Register, retrying transport failures so the node can start before the
        // server is up.
        self.retrying("register", || self.register()).await?;

        loop {
            // One poll-and-maybe-work cycle; transport failures fall back to the
            // reconnect/backoff path and the loop continues.
            match self.poll_once().await {
                Ok(true) => {} // ran a job; immediately poll for the next.
                Ok(false) => self.sleeper.sleep(self.config.idle_poll_delay).await,
                Err(e) if e.is_retryable() => {
                    // Server went away mid-loop: re-register (the lease may have
                    // expired) under backoff, then resume polling.
                    self.retrying("register", || self.register()).await?;
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// Register, then run exactly one poll cycle: claim a job (running it to a
    /// posted result) or observe a `204`. Returns `true` if a job was processed.
    ///
    /// This is the deterministic unit the falsifier drives against a mock server.
    pub async fn run_once(&self) -> Result<bool, AgentError> {
        self.register().await?;
        self.poll_once().await
    }

    /// Send progress for a running job (advisory; `204` expected).
    pub async fn report_progress(&self, job_id: &str, pct: f32) -> Result<(), AgentError> {
        let body = Progress {
            job_id: job_id.to_string(),
            pct,
        };
        let url = format!("{}/jobs/{}/progress", self.base_url, job_id);
        let resp = self.http.post(&url).json(&body).send().await?;
        self.expect_no_content(resp, "progress").await
    }

    /// Heartbeat to keep this node's leases alive.
    pub async fn heartbeat(&self) -> Result<(), AgentError> {
        let body = Heartbeat {
            node_id: self.register.id.clone(),
        };
        let url = format!("{}/nodes/{}/heartbeat", self.base_url, self.register.id);
        let resp = self.http.post(&url).json(&body).send().await?;
        // Heartbeat returns `{"leases": n}`; any 2xx is success.
        if resp.status().is_success() {
            Ok(())
        } else {
            Err(self.status_err("heartbeat", resp.status()))
        }
    }

    /// `POST /nodes/register` — announce capabilities, receive (and ignore here)
    /// the coordination token.
    async fn register(&self) -> Result<NodeRegistered, AgentError> {
        let url = format!("{}/nodes/register", self.base_url);
        let resp = self.http.post(&url).json(&self.register).send().await?;
        if !resp.status().is_success() {
            return Err(self.status_err("register", resp.status()));
        }
        Ok(resp.json::<NodeRegistered>().await?)
    }

    /// One cycle: claim work; if a job comes back, fetch its audio, run it, and
    /// post the result. Returns `true` when a job was processed, `false` on a
    /// `204 No Work`.
    async fn poll_once(&self) -> Result<bool, AgentError> {
        match self.request_work().await? {
            Some(WorkResponse::Work {
                job_id,
                audio_url,
                opts,
                ..
            }) => {
                self.run_job(&job_id, &audio_url, &opts).await?;
                Ok(true)
            }
            // `NoWork` can't appear over the wire (the server answers 204), but
            // handle it for completeness if a server ever inlines it.
            Some(WorkResponse::NoWork) | None => Ok(false),
        }
    }

    /// Fetch audio, run the processor, and report the terminal result. A
    /// processing failure is reported as a failed [`JobResult`] (not an
    /// `AgentError`) so one bad clip never stops the node.
    async fn run_job(&self, job_id: &str, audio_url: &str, opts: &JobOpts) -> Result<(), AgentError> {
        let pcm = self.fetch_audio(audio_url).await?;
        let outcome = match self.processor.process(opts, pcm).await {
            Ok(output) => JobOutcome::Ok { output },
            Err(error) => {
                tracing::warn!(job = job_id, error, "job processing failed");
                JobOutcome::Err { error }
            }
        };
        self.report_result(job_id, outcome).await
    }

    /// `POST /nodes/{id}/request-work` — claim the next job. A `204` (the server's
    /// "nothing to do") maps to `None`.
    async fn request_work(&self) -> Result<Option<WorkResponse>, AgentError> {
        let body = WorkRequest {
            node_id: self.register.id.clone(),
        };
        let url = format!("{}/nodes/{}/request-work", self.base_url, self.register.id);
        let resp = self.http.post(&url).json(&body).send().await?;
        if resp.status() == reqwest::StatusCode::NO_CONTENT {
            return Ok(None);
        }
        if !resp.status().is_success() {
            return Err(self.status_err("request-work", resp.status()));
        }
        Ok(Some(resp.json::<WorkResponse>().await?))
    }

    /// `GET {audio_url}` — pull the job's extracted PCM bytes.
    ///
    /// `audio_url` from a `Work` response is server-relative
    /// (`/jobs/{id}/audio`); it is joined onto the base URL unless already
    /// absolute.
    async fn fetch_audio(&self, audio_url: &str) -> Result<Vec<u8>, AgentError> {
        let url = self.absolute(audio_url);
        let resp = self.http.get(&url).send().await?;
        if !resp.status().is_success() {
            return Err(self.status_err("audio", resp.status()));
        }
        Ok(resp.bytes().await?.to_vec())
    }

    /// `POST /jobs/{id}/result` — report the terminal outcome (`204` expected).
    async fn report_result(&self, job_id: &str, outcome: JobOutcome) -> Result<(), AgentError> {
        let body = JobResult {
            job_id: job_id.to_string(),
            outcome,
        };
        let url = format!("{}/jobs/{}/result", self.base_url, job_id);
        let resp = self.http.post(&url).json(&body).send().await?;
        self.expect_no_content(resp, "result").await
    }

    /// Run `op` under reconnect-with-backoff: retry transport failures with
    /// exponential backoff + jitter until it succeeds; surface any non-transport
    /// error immediately.
    async fn retrying<T, F, Fut>(&self, what: &str, mut op: F) -> Result<T, AgentError>
    where
        F: FnMut() -> Fut,
        Fut: Future<Output = Result<T, AgentError>>,
    {
        let mut attempt: u32 = 0;
        loop {
            match op().await {
                Ok(value) => return Ok(value),
                Err(e) if e.is_retryable() => {
                    let delay = self.backoff_delay(attempt);
                    tracing::warn!(op = what, ?delay, error = %e, "server unavailable, backing off");
                    self.sleeper.sleep(delay).await;
                    attempt = attempt.saturating_add(1);
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// Exponential backoff with jitter for the `attempt`-th (0-based) failure.
    ///
    /// `base * 2^attempt`, capped at `reconnect_max`, then a random fraction in
    /// `[0.5, 1.0)` of that cap is added as jitter so a fleet of nodes recovering
    /// from the same outage does not retry in lockstep (the thundering-herd the
    /// repo's network-retry convention guards against). Jitter uses a cheap
    /// time-seeded PRNG rather than pulling in a `rand` dependency.
    fn backoff_delay(&self, attempt: u32) -> Duration {
        let base = self.config.reconnect_base.as_millis() as u64;
        let cap = self.config.reconnect_max.as_millis() as u64;
        let exp = base.saturating_mul(1u64.checked_shl(attempt.min(20)).unwrap_or(u64::MAX));
        let capped = exp.min(cap);
        let jitter = (jitter_fraction() * capped as f64) as u64;
        Duration::from_millis(capped.saturating_add(jitter))
    }

    /// Join a possibly-relative URL onto the agent's base URL.
    fn absolute(&self, url: &str) -> String {
        if url.starts_with("http://") || url.starts_with("https://") {
            url.to_string()
        } else if let Some(rest) = url.strip_prefix('/') {
            format!("{}/{}", self.base_url, rest)
        } else {
            format!("{}/{}", self.base_url, url)
        }
    }

    /// Treat a `204` as success and any other code as a contract error.
    async fn expect_no_content(&self, resp: reqwest::Response, endpoint: &str) -> Result<(), AgentError> {
        if resp.status() == reqwest::StatusCode::NO_CONTENT || resp.status().is_success() {
            Ok(())
        } else {
            Err(self.status_err(endpoint, resp.status()))
        }
    }

    fn status_err(&self, endpoint: &str, status: reqwest::StatusCode) -> AgentError {
        AgentError::Status {
            endpoint: endpoint.to_string(),
            status,
        }
    }
}

/// Jitter fraction in `[0.5, 1.0)` from a time-seeded xorshift step.
///
/// Cheap and dependency-free: the goal is only to desynchronise reconnecting
/// nodes, not cryptographic randomness.
fn jitter_fraction() -> f64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
        | 1;
    // One xorshift64 step.
    let mut x = seed;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    0.5 + (x as f64 / u64::MAX as f64) * 0.5
}

/// Guards the one-time install of the whisper.cpp logging redirection.
static WHISPER_LOG_HOOK: Once = Once::new();

/// Counts how many times the logging redirection was actually installed.
///
/// The structural falsifier reads this to confirm the hook installs exactly
/// once no matter how many nodes/processors are built. It increments only inside
/// the [`Once`], so it can never exceed 1.
static WHISPER_LOG_HOOK_INSTALLS: AtomicUsize = AtomicUsize::new(0);

/// Route whisper.cpp/ggml stderr spam (`whisper_full_with_state`, `seek=…`,
/// `ggml_…`, `system_info`) through `tracing` instead of raw C stderr.
///
/// whisper.cpp installs a process-global log callback, so this must run exactly
/// once; the [`Once`] makes repeated calls (one per node/model init) a no-op.
/// With the redirection in place the C library's chatter becomes `tracing`
/// events — hidden at the default `INFO` level and surfaced only at
/// `--log-level DEBUG` — so a normal transcribe run no longer floods the
/// terminal.
///
/// Defined regardless of the `model` feature so the install path stays testable
/// without linking whisper.cpp; the actual `whisper_rs` call is gated on
/// `model`, the only build that has a C library to quiet.
pub fn install_whisper_logging() {
    WHISPER_LOG_HOOK.call_once(|| {
        #[cfg(feature = "model")]
        whisper_rs::install_whisper_tracing_trampoline();
        WHISPER_LOG_HOOK_INSTALLS.fetch_add(1, Ordering::SeqCst);
    });
}

/// How many times [`install_whisper_logging`] has installed the redirection
/// (`0` before the first call, `1` forever after). Exposed for the structural
/// falsifier that pins the once-only install at model-init.
pub fn whisper_logging_install_count() -> usize {
    WHISPER_LOG_HOOK_INSTALLS.load(Ordering::SeqCst)
}

/// Build a [`JobProcessor`] that decodes the server's PCM and transcribes it
/// through the [`Dispatcher`] into a subtitle string.
///
/// Available with the `model` feature, which links whisper.cpp. The dispatcher
/// holds a runner permit across the whole inference so per-node concurrency
/// stays capped; the subtitle assembly (regroup / output) is the
/// `submate-subtitle` slice and is wired in by its own backlog item.
///
/// Building the processor also installs the whisper.cpp logging redirection
/// (once per process) so the C library's stderr spam routes through `tracing`
/// rather than the terminal.
#[cfg(feature = "model")]
pub fn whisper_processor(
    dispatcher: Dispatcher,
    model_path: impl Into<String>,
) -> impl JobProcessor {
    install_whisper_logging();
    let model_path = model_path.into();
    move |opts: &JobOpts, pcm: Vec<u8>| {
        let dispatcher = dispatcher.clone();
        let model_path = model_path.clone();
        let language = opts.source_language.clone();
        async move {
            let samples = pcm_s16le_to_f32(&pcm);
            let options = submate_whisper::TranscribeOptions {
                language,
                task: submate_whisper::Task::Transcribe,
            };
            // Decode, then run the full subtitle assembly (regroup -> suppress ->
            // SRT formatting) so the job output is a real timestamped SRT, not raw
            // text. The assembly stages are the stable-ts slice (already ported).
            let raw = dispatcher
                .transcribe_pcm(model_path, samples.clone(), options)
                .await
                .map_err(|e| e.to_string())?;
            let assembled =
                submate_whisper::assemble_result(&raw, submate_whisper::DEFAULT_REGROUP, &samples)
                    .map_err(|e| e.to_string())?;
            Ok(assembled.to_srt_vtt(false))
        }
    }
}

/// Decode little-endian s16 PCM bytes into normalized f32 samples in `-1.0..=1.0`.
#[cfg(feature = "model")]
fn pcm_s16le_to_f32(pcm: &[u8]) -> Vec<f32> {
    pcm.chunks_exact(2)
        .map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.0)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Condvar, Mutex};
    use std::time::Duration;

    use tokio::time::timeout;

    fn lang_result(language: &str) -> WhisperResult {
        WhisperResult {
            language: language.to_string(),
            text: String::new(),
            segments: Vec::new(),
        }
    }

    /// A gate the blocking jobs park on synchronously (they run off the async
    /// runtime, so they use std primitives, not tokio ones). The test opens the
    /// gate once it has confirmed the third job is still waiting for a permit.
    #[derive(Default)]
    struct Gate {
        open: Mutex<bool>,
        cv: Condvar,
    }

    impl Gate {
        fn wait(&self) {
            let mut open = self.open.lock().unwrap();
            while !*open {
                open = self.cv.wait(open).unwrap();
            }
        }

        fn release(&self) {
            *self.open.lock().unwrap() = true;
            self.cv.notify_all();
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn dispatcher_caps_concurrency() {
        let dispatcher = Dispatcher::new(2);

        // Counters observe how many jobs are inside the blocking step at once.
        let active = Arc::new(AtomicUsize::new(0));
        let max_active = Arc::new(AtomicUsize::new(0));
        let started = Arc::new(AtomicUsize::new(0));
        let gate = Arc::new(Gate::default());

        let spawn = |id: usize| {
            let dispatcher = dispatcher.clone();
            let active = Arc::clone(&active);
            let max_active = Arc::clone(&max_active);
            let started = Arc::clone(&started);
            let gate = Arc::clone(&gate);
            tokio::spawn(async move {
                dispatcher
                    .transcribe_with(move || {
                        started.fetch_add(1, Ordering::SeqCst);
                        let now = active.fetch_add(1, Ordering::SeqCst) + 1;
                        max_active.fetch_max(now, Ordering::SeqCst);
                        // Park inside the blocking step (and thus while holding a
                        // permit) until the test opens the gate.
                        gate.wait();
                        active.fetch_sub(1, Ordering::SeqCst);
                        Ok(lang_result(&format!("lang{id}")))
                    })
                    .await
            })
        };

        let h1 = spawn(1);
        let h2 = spawn(2);
        let h3 = spawn(3);

        // Wait until two jobs are parked holding permits, then confirm the third
        // is still blocked: only `runners` (2) permits exist, so exactly two can
        // be inside the blocking step. If the cap leaked, all three would start.
        let two_running = timeout(Duration::from_secs(5), async {
            loop {
                if started.load(Ordering::SeqCst) >= 2 {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await;
        assert!(two_running.is_ok(), "first two jobs never both started");

        // Give a leaked third job a chance to also start before we assert.
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(
            started.load(Ordering::SeqCst),
            2,
            "a third job ran while both permits were held — concurrency cap leaked"
        );
        assert_eq!(dispatcher.available_permits(), 0);

        // Release every parked job. As the first two drain they free permits and
        // the third finally acquires one and runs.
        gate.release();
        let results = timeout(Duration::from_secs(5), async {
            let r1 = h1.await.expect("task 1 panicked");
            let r2 = h2.await.expect("task 2 panicked");
            let r3 = h3.await.expect("task 3 panicked");
            (r1, r2, r3)
        })
        .await
        .expect("dispatcher deadlocked or starved a permit");

        // Results return correctly for all three submissions.
        let (r1, r2, r3) = results;
        let langs: Vec<String> = [r1, r2, r3]
            .into_iter()
            .map(|r| r.expect("transcription failed").language)
            .collect();
        for want in ["lang1", "lang2", "lang3"] {
            assert!(langs.contains(&want.to_string()), "missing result {want}");
        }

        // Never more than `runners` jobs ran the blocking step at once.
        assert!(
            max_active.load(Ordering::SeqCst) <= 2,
            "concurrency exceeded the runner cap: saw {} active",
            max_active.load(Ordering::SeqCst)
        );
        // Permits are all returned after the work drains.
        assert_eq!(dispatcher.available_permits(), 2);
    }

    #[tokio::test]
    async fn runners_reports_configured_count() {
        let dispatcher = Dispatcher::new(3);
        assert_eq!(dispatcher.runners(), 3);
        assert_eq!(dispatcher.available_permits(), 3);
    }

    #[tokio::test]
    async fn errors_propagate_and_release_permit() {
        let dispatcher = Dispatcher::new(1);
        let result = dispatcher
            .transcribe_with(|| Err(WhisperError::Inference("boom".into())))
            .await;
        assert!(matches!(result, Err(WhisperError::Inference(_))));
        // The permit is returned even when the job errors.
        assert_eq!(dispatcher.available_permits(), 1);
    }

    #[tokio::test]
    #[should_panic(expected = "at least one runner")]
    async fn zero_runners_panics() {
        let _ = Dispatcher::new(0);
    }
}

#[cfg(test)]
mod logging_tests {
    use super::*;

    /// Falsifier: the whisper.cpp logging redirection installs exactly once,
    /// even though the install runs at every node/model init. whisper.cpp's log
    /// callback is process-global, so a second install would re-register it; the
    /// [`Once`] guard must collapse repeated calls into a single install.
    ///
    /// Pins the structural wiring without a model file: the test exercises the
    /// install path directly and asserts the install counter saturates at 1. Full
    /// terminal silence is confirmed by a human run; here we lock in the
    /// once-only contract the model-init path relies on.
    #[test]
    fn whisper_logging_hooked() {
        // First install crosses the Once and bumps the counter to exactly 1.
        install_whisper_logging();
        assert_eq!(
            whisper_logging_install_count(),
            1,
            "first install should hook the whisper.cpp logger exactly once",
        );

        // Every later call (a second node, another model init) is a no-op: the
        // process-global callback stays installed and the counter never grows.
        for _ in 0..5 {
            install_whisper_logging();
        }
        assert_eq!(
            whisper_logging_install_count(),
            1,
            "repeated installs must not re-register the whisper.cpp log callback",
        );
    }
}

#[cfg(test)]
mod agent_tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex as StdMutex;

    use submate_types::{Device, TranscriptionTask, WhisperModel};
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, Request, ResponseTemplate};

    fn test_register() -> NodeRegister {
        NodeRegister {
            id: "node-1".into(),
            gpu: false,
            runners: 1,
            tasks: vec![TranscriptionTask::Transcribe],
        }
    }

    fn job_opts() -> JobOpts {
        JobOpts {
            model: WhisperModel::Medium,
            device: Device::Cpu,
            source_language: None,
            target_language: None,
            translation_backend: None,
        }
    }

    /// A `Sleeper` that returns immediately and records the delays it was asked
    /// to wait, so the backoff loop runs without real time passing.
    #[derive(Clone, Default)]
    struct RecordingSleeper {
        delays: Arc<StdMutex<Vec<Duration>>>,
    }

    impl Sleeper for RecordingSleeper {
        fn sleep(&self, dur: Duration) -> impl Future<Output = ()> + Send {
            self.delays.lock().unwrap().push(dur);
            std::future::ready(())
        }
    }

    /// Falsifier: against a wiremock server the agent registers, claims one job,
    /// fetches its audio, runs the processor, and POSTs the result. Wiring the
    /// `request-work` mock to a one-shot job followed by `204` proves the loop
    /// processes a job and then long-polls again on the empty poll.
    #[tokio::test]
    async fn agent_pull_loop() {
        let server = MockServer::start().await;

        // register → token
        Mock::given(method("POST"))
            .and(path("/nodes/register"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(NodeRegistered {
                    token: "tok-xyz".into(),
                }),
            )
            .mount(&server)
            .await;

        // First request-work hands out a job; thereafter 204 (nothing to do).
        let work_calls = Arc::new(AtomicUsize::new(0));
        let work_audio_url = format!("{}/jobs/job-1/audio", server.uri());
        {
            let work_calls = Arc::clone(&work_calls);
            let opts = job_opts();
            Mock::given(method("POST"))
                .and(path("/nodes/node-1/request-work"))
                .respond_with(move |_req: &Request| {
                    let n = work_calls.fetch_add(1, Ordering::SeqCst);
                    if n == 0 {
                        ResponseTemplate::new(200).set_body_json(WorkResponse::Work {
                            job_id: "job-1".into(),
                            kind: TranscriptionTask::Transcribe,
                            audio_url: work_audio_url.clone(),
                            opts: opts.clone(),
                        })
                    } else {
                        // 204: the long-poll found nothing — the loop polls again.
                        ResponseTemplate::new(204)
                    }
                })
                .mount(&server)
                .await;
        }

        // audio → raw PCM bytes the processor receives verbatim.
        let audio_bytes = vec![1u8, 2, 3, 4];
        Mock::given(method("GET"))
            .and(path("/jobs/job-1/audio"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(audio_bytes.clone()))
            .mount(&server)
            .await;

        // result → 204; captured to assert the agent POSTed the processor output.
        let posted_result = Arc::new(StdMutex::new(None::<JobResult>));
        {
            let posted_result = Arc::clone(&posted_result);
            Mock::given(method("POST"))
                .and(path("/jobs/job-1/result"))
                .respond_with(move |req: &Request| {
                    let body: JobResult = req.body_json().unwrap();
                    *posted_result.lock().unwrap() = Some(body);
                    ResponseTemplate::new(204)
                })
                .mount(&server)
                .await;
        }

        // Processor: assert it sees the fetched PCM, return a canned subtitle.
        let seen_pcm = Arc::new(StdMutex::new(Vec::<u8>::new()));
        let processor = {
            let seen_pcm = Arc::clone(&seen_pcm);
            move |_opts: &JobOpts, pcm: Vec<u8>| {
                let seen_pcm = Arc::clone(&seen_pcm);
                async move {
                    *seen_pcm.lock().unwrap() = pcm;
                    Ok::<_, String>("1\n00:00:00,000 --> 00:00:01,000\nhello\n".to_string())
                }
            }
        };

        let agent = Agent::new(server.uri(), test_register(), Dispatcher::new(1), processor);

        // Cycle 1: register + claim + process + result.
        let ran = agent.run_once().await.expect("first cycle failed");
        assert!(ran, "agent should have processed a job");

        // The processor received exactly the bytes the audio endpoint served.
        assert_eq!(*seen_pcm.lock().unwrap(), audio_bytes);

        // The posted result carried the processor's subtitle output.
        let result = posted_result.lock().unwrap().clone().expect("no result posted");
        assert_eq!(result.job_id, "job-1");
        assert!(
            matches!(result.outcome, JobOutcome::Ok { output } if output.contains("hello")),
            "result did not carry the processor output",
        );

        // Cycle 2: now request-work returns 204 — the agent long-polls again and
        // reports no job (the "on a 204 it long-polls again" branch).
        let ran_again = agent.poll_once().await.expect("second poll failed");
        assert!(!ran_again, "204 poll should not process a job");
        assert!(work_calls.load(Ordering::SeqCst) >= 2, "second poll never hit request-work");
    }

    /// A processing failure is reported back as a failed `JobResult` (ok:false),
    /// not propagated as an `AgentError` — one bad clip must not stop the node.
    #[tokio::test]
    async fn processing_failure_posts_failed_result() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/nodes/register"))
            .respond_with(ResponseTemplate::new(200).set_body_json(NodeRegistered { token: "t".into() }))
            .mount(&server)
            .await;

        let audio_url = format!("{}/jobs/job-9/audio", server.uri());
        let opts = job_opts();
        Mock::given(method("POST"))
            .and(path("/nodes/node-1/request-work"))
            .respond_with(move |_req: &Request| {
                ResponseTemplate::new(200).set_body_json(WorkResponse::Work {
                    job_id: "job-9".into(),
                    kind: TranscriptionTask::Transcribe,
                    audio_url: audio_url.clone(),
                    opts: opts.clone(),
                })
            })
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/jobs/job-9/audio"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(vec![0u8; 4]))
            .mount(&server)
            .await;

        let posted = Arc::new(StdMutex::new(None::<JobResult>));
        {
            let posted = Arc::clone(&posted);
            Mock::given(method("POST"))
                .and(path("/jobs/job-9/result"))
                .respond_with(move |req: &Request| {
                    *posted.lock().unwrap() = Some(req.body_json().unwrap());
                    ResponseTemplate::new(204)
                })
                .mount(&server)
                .await;
        }

        let processor = |_opts: &JobOpts, _pcm: Vec<u8>| async {
            Err::<String, String>("model load failed".to_string())
        };
        let agent = Agent::new(server.uri(), test_register(), Dispatcher::new(1), processor);

        let ran = agent.run_once().await.expect("cycle failed");
        assert!(ran, "a job was claimed even though processing failed");

        let result = posted.lock().unwrap().clone().expect("no result posted");
        assert!(
            matches!(result.outcome, JobOutcome::Err { error } if error.contains("model load failed")),
            "failure was not reported as a failed JobResult",
        );
    }

    /// On a transport failure (server down) the run loop reconnects with
    /// exponential backoff: `register` is retried, the recorded delays grow, and
    /// each stays within the configured cap (plus its ≤100% jitter).
    #[tokio::test]
    async fn reconnect_backs_off_when_server_unavailable() {
        // Point at a closed port so every register attempt is a transport error.
        let dead_url = "http://127.0.0.1:1";

        let sleeper = RecordingSleeper::default();
        let config = AgentConfig {
            reconnect_base: Duration::from_millis(100),
            reconnect_max: Duration::from_millis(800),
            idle_poll_delay: Duration::from_millis(10),
        };
        let processor = |_opts: &JobOpts, _pcm: Vec<u8>| async { Ok::<_, String>(String::new()) };
        let agent =
            Agent::with_config(dead_url, test_register(), Dispatcher::new(1), processor, config)
                .with_sleeper(sleeper.clone());

        // The register retry loop never returns against a dead server, so drive
        // it under a timeout and inspect the backoff schedule it recorded.
        let _ = tokio::time::timeout(Duration::from_secs(2), agent.run()).await;

        let delays = sleeper.delays.lock().unwrap().clone();
        assert!(delays.len() >= 3, "expected several backoff sleeps, got {}", delays.len());

        // Exponential growth: base*2^attempt capped at reconnect_max, plus jitter
        // up to that capped value (so at most 2x the cap).
        let cap = config.reconnect_max;
        for (attempt, delay) in delays.iter().take(4).enumerate() {
            let base = config.reconnect_base.as_millis() as u64;
            let expected = (base << attempt.min(20)).min(cap.as_millis() as u64);
            assert!(
                *delay >= Duration::from_millis(expected),
                "attempt {attempt}: delay {delay:?} below capped base {expected}ms",
            );
            assert!(
                *delay <= cap.saturating_mul(2),
                "attempt {attempt}: delay {delay:?} exceeded cap+jitter",
            );
        }
    }
}
