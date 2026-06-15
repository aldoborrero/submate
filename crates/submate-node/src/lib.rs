//! Processing-node agent: pull work from the server, run the local Whisper/translation dispatcher, report results.
//!
//! The dispatcher is the per-node execution core. It holds a [`tokio::sync::Semaphore`]
//! sized to the node's runner count and gates every transcription behind a permit,
//! so at most `runners` clips transcribe concurrently — the rest wait for a permit
//! to free. This is the in-process concurrency cap.
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
    Heartbeat, JobOpts, JobOutcome, JobResult, NodeRegister, NodeRegistered, OutputFormat,
    Progress, WorkRequest, WorkResponse,
};
use submate_translate::Backend;
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
        matches!(self, Self::Http(_))
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

/// The optional translation post-transcription step the node applies before
/// reporting a job's result.
///
/// Lives in the job layer (not in [`whisper_processor`]) so it composes with any
/// [`JobProcessor`] and is testable without the `model` feature: a job that
/// carries a `target_language` has its assembled subtitle translated here, while
/// a job without one is reported byte-for-byte as the processor produced it.
///
/// Holds the configured translation [`Backend`] plus the chunk size from config;
/// `translate` dispatches on the job's [`OutputFormat`] into the matching
/// `submate-translate` entry point.
pub struct TranslationStep {
    // The async backends `.await` their `reqwest::Client` directly on the
    // runtime, so the step is held by reference across `run_job`'s `.await`
    // without any `spawn_blocking`/`Arc` shuffle.
    backend: Box<dyn Backend + Send + Sync>,
    chunk_size: usize,
}

impl TranslationStep {
    /// Build a step from a configured backend and the config `chunk_size`.
    pub fn new(backend: Box<dyn Backend + Send + Sync>, chunk_size: usize) -> Self {
        Self {
            backend,
            chunk_size,
        }
    }

    /// Translate `subtitle` for `opts`, or return it unchanged when no
    /// translation is requested.
    ///
    /// Returns `subtitle` verbatim when `opts.target_language` is `None`, so the
    /// plain-transcription path is byte-identical to a node with no translation
    /// step. Otherwise the source language is `opts.source_language` (falling
    /// back to `"auto"` when the decode hint was auto-detect) and the target is
    /// the requested language; the dispatch is by `opts.output_format`:
    ///
    /// * [`OutputFormat::Srt`] → [`submate_translate::translate_srt_content`]
    /// * [`OutputFormat::Vtt`] → [`submate_translate::translate_vtt_content`]
    /// * [`OutputFormat::Ass`] → per-`Dialogue`-line
    ///   [`submate_translate::translate_ass_dialogue`]
    ///
    /// [`OutputFormat::Json`] / [`OutputFormat::Txt`] carry no cue structure to
    /// translate in place, so they pass through unchanged.
    ///
    /// A backend error is surfaced as the `String` error the agent reports as a
    /// failed [`JobResult`].
    pub async fn translate(&self, opts: &JobOpts, subtitle: String) -> Result<String, String> {
        let Some(target) = opts.target_language.as_deref() else {
            return Ok(subtitle);
        };
        let source = opts.source_language.as_deref().unwrap_or("auto");

        let mut complete = async |prompt: String| {
            self.backend
                .complete(&prompt)
                .await
                .map_err(|e| e.to_string())
        };

        match opts.output_format {
            OutputFormat::Srt => {
                submate_translate::translate_srt_content(
                    &subtitle,
                    source,
                    target,
                    self.chunk_size,
                    &mut complete,
                )
                .await
            }
            OutputFormat::Vtt => {
                submate_translate::translate_vtt_content(
                    &subtitle,
                    source,
                    target,
                    self.chunk_size,
                    &mut complete,
                )
                .await
            }
            OutputFormat::Ass => {
                translate_ass_content(&subtitle, source, target, self.chunk_size, &mut complete)
                    .await
            }
            // No cue structure to translate in place; leave untouched.
            OutputFormat::Json | OutputFormat::Txt => Ok(subtitle),
        }
    }
}

/// Translate the dialogue text of an ASS document in place, preserving every
/// non-`Dialogue` line, the event field layout, and the override tags.
///
/// The workspace has no ASS (de)serializer, so this walks the `[Events]` lines:
/// each `Dialogue:` line's text is the 10th comma-separated field (the nine
/// leading fields — `Layer,Start,End,Style,Name,MarginL,MarginR,MarginV,Effect`
/// — never contain a comma in well-formed output, and the text field keeps any
/// commas it contains). The text fields are translated as a batch through
/// [`submate_translate::translate_ass_dialogue`], which drops any translation
/// that would alter the line's `{...}` tags, then spliced back onto their lines.
async fn translate_ass_content<E, F, Fut>(
    ass: &str,
    source_lang: &str,
    target_lang: &str,
    chunk_size: usize,
    complete: &mut F,
) -> Result<String, E>
where
    F: FnMut(String) -> Fut,
    Fut: Future<Output = Result<String, E>>,
{
    // Record (line index, byte offset where the text field begins) for every
    // Dialogue line, and collect the dialogue texts to translate together.
    let mut dialogue: Vec<(usize, usize)> = Vec::new();
    let mut texts: Vec<String> = Vec::new();
    for (idx, line) in ass.lines().enumerate() {
        if let Some(rest) = line.strip_prefix("Dialogue:") {
            // The text is everything after the 9th comma in `rest`.
            if let Some(text_start) = nth_comma_end(rest, 9) {
                dialogue.push((idx, "Dialogue:".len() + text_start));
                texts.push(rest[text_start..].to_string());
            }
        }
    }

    if texts.is_empty() {
        return Ok(ass.to_string());
    }

    let translated = submate_translate::translate_ass_dialogue(
        &texts,
        source_lang,
        target_lang,
        chunk_size,
        complete,
    )
    .await?;

    // Map each translated Dialogue line by its line index, then rebuild the
    // document line by line — swapping the text field on Dialogue lines and
    // copying every other line verbatim. `split_inclusive('\n')` keeps each
    // line's own newline, so the output is byte-stable apart from the swapped
    // text (no trailing newline is invented or dropped).
    use std::collections::HashMap;
    let new_texts: HashMap<usize, (usize, String)> = dialogue
        .into_iter()
        .zip(translated)
        .map(|((idx, offset), text)| (idx, (offset, text)))
        .collect();

    let mut out = String::with_capacity(ass.len());
    for (idx, raw) in ass.split_inclusive('\n').enumerate() {
        match new_texts.get(&idx) {
            Some((text_offset, new_text)) => {
                let (line, ending) = split_line_ending(raw);
                out.push_str(&line[..*text_offset]);
                out.push_str(new_text);
                out.push_str(ending);
            }
            None => out.push_str(raw),
        }
    }
    Ok(out)
}

/// Byte offset, within `s`, just past the `n`-th comma (so `s[offset..]` is the
/// remainder after `n` separators). Returns `None` when fewer than `n` commas
/// are present.
fn nth_comma_end(s: &str, n: usize) -> Option<usize> {
    let mut seen = 0;
    for (i, b) in s.bytes().enumerate() {
        if b == b',' {
            seen += 1;
            if seen == n {
                return Some(i + 1);
            }
        }
    }
    None
}

/// Split a `split_inclusive('\n')` chunk into its content and trailing newline
/// (`""` when the final chunk has none), so the content can be edited without
/// disturbing the line ending.
fn split_line_ending(raw: &str) -> (&str, &str) {
    match raw.strip_suffix('\n') {
        Some(content) => (content, "\n"),
        None => (raw, ""),
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
    /// Optional translation post-step: when present, a job carrying a
    /// `target_language` has its assembled subtitle translated before the result
    /// is reported. `None` means the node only transcribes.
    translation: Option<TranslationStep>,
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
    pub fn new(
        base_url: impl Into<String>,
        register: NodeRegister,
        dispatcher: Dispatcher,
        processor: P,
    ) -> Self {
        Self::with_config(
            base_url,
            register,
            dispatcher,
            processor,
            AgentConfig::default(),
        )
    }

    /// Like [`new`](Agent::new) but with explicit backoff/poll [`AgentConfig`].
    pub fn with_config(
        base_url: impl Into<String>,
        register: NodeRegister,
        dispatcher: Dispatcher,
        processor: P,
        config: AgentConfig,
    ) -> Self {
        Self {
            http: reqwest::Client::new(),
            base_url: base_url.into().trim_end_matches('/').to_string(),
            register,
            dispatcher,
            processor,
            translation: None,
            config,
            sleeper: TokioSleeper,
        }
    }

    /// Attach the translation post-step: jobs carrying a `target_language` will
    /// have their subtitle translated through `step` before the result is
    /// reported. Jobs without a `target_language` are unaffected.
    pub fn with_translation(mut self, step: TranslationStep) -> Self {
        self.translation = Some(step);
        self
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
            translation: self.translation,
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
    async fn run_job(
        &self,
        job_id: &str,
        audio_url: &str,
        opts: &JobOpts,
    ) -> Result<(), AgentError> {
        let pcm = self.fetch_audio(audio_url).await?;
        // Transcribe, then (only when a translation step is configured and the
        // job carries a `target_language`) translate the assembled subtitle
        // before reporting. With no `target_language` the post-step returns the
        // processor output byte-for-byte, so plain transcription is unaffected.
        let result = match self.processor.process(opts, pcm).await {
            Ok(output) => match &self.translation {
                // The async backends `.await` their `reqwest::Client` directly on
                // the runtime, so the translate runs inline here.
                Some(step) => step.translate(opts, output).await,
                None => Ok(output),
            },
            Err(error) => Err(error),
        };
        let outcome = match result {
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
    async fn expect_no_content(
        &self,
        resp: reqwest::Response,
        endpoint: &str,
    ) -> Result<(), AgentError> {
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
        .map_or(0, |d| d.as_nanos() as u64)
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
        whisper_rs::install_logging_hooks();
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
/// `submate-subtitle` slice.
///
/// Building the processor also installs the whisper.cpp logging redirection
/// (once per process) so the C library's stderr spam routes through `tracing`
/// rather than the terminal.
#[cfg(feature = "model")]
pub fn whisper_processor(
    dispatcher: Dispatcher,
    model_path: impl Into<String>,
) -> impl JobProcessor {
    use submate_proto::OutputFormat;

    install_whisper_logging();
    let model_path = model_path.into();
    move |opts: &JobOpts, pcm: Vec<u8>| {
        let dispatcher = dispatcher.clone();
        let model_path = model_path.clone();
        let language = opts.source_language.clone();
        let output_format = opts.output_format;
        let initial_prompt = opts.initial_prompt.clone();
        let beam_size = opts.beam_size;
        let temperature = opts.temperature;
        let no_speech_threshold = opts.no_speech_threshold;
        let entropy_threshold = opts.entropy_threshold;
        let logprob_threshold = opts.logprob_threshold;
        let max_len = opts.max_len;
        async move {
            let samples = pcm_s16le_to_f32(&pcm);
            let options = submate_whisper::TranscribeOptions {
                language,
                task: submate_whisper::Task::Transcribe,
                initial_prompt,
                beam_size,
                temperature,
                no_speech_threshold,
                entropy_threshold,
                logprob_threshold,
                max_len,
            };
            // Decode, then run the full subtitle assembly (regroup -> suppress ->
            // output formatting) so the job output is a real assembled result, not
            // raw text. The assembly stages are the stable-ts slice; the final
            // serialization honors the job's requested format.
            let raw = dispatcher
                .transcribe_pcm(model_path, samples.clone(), options)
                .await
                .map_err(|e| e.to_string())?;
            let assembled =
                submate_whisper::assemble_result(&raw, submate_whisper::DEFAULT_REGROUP, &samples)
                    .map_err(|e| e.to_string())?;
            Ok(match output_format {
                OutputFormat::Srt => assembled.to_srt_vtt(false),
                OutputFormat::Vtt => assembled.to_srt_vtt(true),
                OutputFormat::Ass => assembled.to_ass(),
                OutputFormat::Json => assembled.to_json(),
                OutputFormat::Txt => assembled.to_txt(),
            })
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
mod translation_tests {
    use super::*;

    use submate_proto::OutputFormat;
    use submate_translate::{Backend, BackendError};
    use submate_types::{Device, WhisperModel};

    /// A stub backend that deterministically transforms the prompt: it returns
    /// every line of the prompt after the `Text to translate:` / `Subtitles to
    /// translate:` marker, uppercased and re-joined with the batch separator —
    /// so a translated cue is its source text uppercased, with no network.
    struct UpperBackend;

    #[async_trait::async_trait]
    impl Backend for UpperBackend {
        fn id(&self) -> &'static str {
            "upper"
        }

        async fn complete(&self, prompt: &str) -> Result<String, BackendError> {
            // The chunking layer joins cues with a separator token and embeds
            // them after the prompt's marker; echo the joined batch back
            // uppercased so the per-cue split realigns 1:1.
            let marker = prompt
                .rfind("translate:\n")
                .map_or(0, |i| i + "translate:\n".len());
            Ok(prompt[marker..].to_uppercase())
        }
    }

    fn job(target: Option<&str>, format: OutputFormat) -> JobOpts {
        JobOpts {
            model: WhisperModel::Medium,
            device: Device::Cpu,
            source_language: Some("en".into()),
            target_language: target.map(str::to_string),
            translation_backend: None,
            output_format: format,
            initial_prompt: None,
            beam_size: None,
            temperature: None,
            no_speech_threshold: None,
            entropy_threshold: None,
            logprob_threshold: None,
            max_len: None,
        }
    }

    const FIXED_SRT: &str =
        "1\n00:00:00,000 --> 00:00:01,000\nhello\n\n2\n00:00:01,000 --> 00:00:02,000\nworld\n\n";

    /// Falsifier: the job-layer translation post-step transforms cue text when a
    /// `target_language` is set and is a no-op otherwise — driven with a stub
    /// backend that uppercases each batch, no `model` feature, no network.
    #[tokio::test]
    async fn translate_post_step() {
        let step = TranslationStep::new(Box::new(UpperBackend), 50);

        // target_language = Some -> cue text is transformed, timing preserved.
        let translated = step
            .translate(&job(Some("es"), OutputFormat::Srt), FIXED_SRT.to_string())
            .await
            .expect("translation succeeds");
        assert!(
            translated.contains("HELLO") && translated.contains("WORLD"),
            "cue text was not transformed: {translated:?}",
        );
        assert!(
            translated.contains("00:00:00,000 --> 00:00:01,000"),
            "timing was not preserved: {translated:?}",
        );

        // target_language = None -> the fixed SRT is returned byte-for-byte.
        let untouched = step
            .translate(&job(None, OutputFormat::Srt), FIXED_SRT.to_string())
            .await
            .expect("no-op translation succeeds");
        assert_eq!(
            untouched, FIXED_SRT,
            "no target language must leave the subtitle unchanged",
        );
    }

    /// The ASS dispatch translates each `Dialogue:` line's text field while
    /// preserving every other line and the event field layout.
    #[tokio::test]
    async fn translate_post_step_ass_preserves_layout() {
        let ass = "[Events]\n\
            Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n\
            Dialogue: 0,0:00:00.00,0:00:01.00,Default,,0,0,0,,hello\n\
            Dialogue: 0,0:00:01.00,0:00:02.00,Default,,0,0,0,,world\n";
        let step = TranslationStep::new(Box::new(UpperBackend), 50);
        let out = step
            .translate(&job(Some("es"), OutputFormat::Ass), ass.to_string())
            .await
            .expect("ass translation succeeds");

        // Header lines and the field prefix are untouched; only the text changes.
        assert!(out.contains("[Events]"));
        assert!(out.contains("Format: Layer, Start, End"));
        assert!(out.contains("Dialogue: 0,0:00:00.00,0:00:01.00,Default,,0,0,0,,HELLO"));
        assert!(out.contains("Dialogue: 0,0:00:01.00,0:00:02.00,Default,,0,0,0,,WORLD"));
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
    use std::sync::Mutex as StdMutex;
    use std::sync::atomic::{AtomicUsize, Ordering};

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
            output_format: submate_proto::OutputFormat::default(),
            initial_prompt: None,
            beam_size: None,
            temperature: None,
            no_speech_threshold: None,
            entropy_threshold: None,
            logprob_threshold: None,
            max_len: None,
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
            .respond_with(ResponseTemplate::new(200).set_body_json(NodeRegistered {
                token: "tok-xyz".into(),
            }))
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
        let result = posted_result
            .lock()
            .unwrap()
            .clone()
            .expect("no result posted");
        assert_eq!(result.job_id, "job-1");
        assert!(
            matches!(result.outcome, JobOutcome::Ok { output } if output.contains("hello")),
            "result did not carry the processor output",
        );

        // Cycle 2: now request-work returns 204 — the agent long-polls again and
        // reports no job (the "on a 204 it long-polls again" branch).
        let ran_again = agent.poll_once().await.expect("second poll failed");
        assert!(!ran_again, "204 poll should not process a job");
        assert!(
            work_calls.load(Ordering::SeqCst) >= 2,
            "second poll never hit request-work"
        );
    }

    /// A processing failure is reported back as a failed `JobResult` (ok:false),
    /// not propagated as an `AgentError` — one bad clip must not stop the node.
    #[tokio::test]
    async fn processing_failure_posts_failed_result() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/nodes/register"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(NodeRegistered { token: "t".into() }),
            )
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
        let agent = Agent::with_config(
            dead_url,
            test_register(),
            Dispatcher::new(1),
            processor,
            config,
        )
        .with_sleeper(sleeper.clone());

        // The register retry loop never returns against a dead server, so drive
        // it under a timeout and inspect the backoff schedule it recorded.
        let _ = tokio::time::timeout(Duration::from_secs(2), agent.run()).await;

        let delays = sleeper.delays.lock().unwrap().clone();
        assert!(
            delays.len() >= 3,
            "expected several backoff sleeps, got {}",
            delays.len()
        );

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
