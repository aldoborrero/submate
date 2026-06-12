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

use std::sync::Arc;

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
