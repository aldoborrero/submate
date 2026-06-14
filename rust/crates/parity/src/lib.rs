//! Parity test helpers.
//!
//! The port is verified by diffing Rust output against *golden* output
//! captured from the existing Python `submate` (see `rust/fixtures/`). Two
//! diff modes exist, and picking the right one per layer is the whole point:
//!
//! * **Exact** ([`assert_json_eq`], [`assert_str_eq`]) — for deterministic
//!   pure-data layers: config resolution, the language table, paths, subtitle
//!   detection, mocked-LLM translation, and *all* of stable-ts regroup (B) and
//!   output (D). The Rust value MUST equal the Python golden byte-for-byte.
//! * **Float-tolerant** ([`assert_f32_close`]) — for the suppress-silence DSP
//!   (C): same `audio.f32` in, word timings out; deterministic math, compared
//!   at a tight epsilon.
//! * **Structural-within-tolerance** ([`assert_segments_close`]) — for full
//!   transcription only, where whisper.cpp ≠ faster-whisper and byte-equality
//!   is impossible. Compares segment count, per-segment timing, and text
//!   similarity within an explicit [`SegTol`].
//!
//! Fixtures resolve relative to `rust/fixtures/`. Tests reference them by
//! sub-path, e.g. `golden("config/defaults.resolved.json")`.

use std::path::{Path, PathBuf};

use serde_json::Value;

/// Absolute path to the committed `rust/fixtures/` directory.
///
/// Resolved from this crate's manifest dir (`rust/crates/parity`) so tests
/// work regardless of the cwd the test runner chooses.
pub fn fixtures_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .canonicalize()
        .unwrap_or_else(|_| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures"))
}

/// Resolve a fixture sub-path to an absolute path under `rust/fixtures/`.
pub fn fixture_path(rel: &str) -> PathBuf {
    fixtures_root().join(rel)
}

/// Load and parse a golden JSON fixture.
///
/// Panics with a clear message if the fixture is missing or malformed — a
/// missing golden is a setup error the capture scripts (`fixtures/capture/`)
/// must fix, not a test failure to paper over.
pub fn golden(rel: &str) -> Value {
    let path = fixture_path(rel);
    let bytes = std::fs::read(&path)
        .unwrap_or_else(|e| panic!("missing golden fixture {}: {e}", path.display()));
    serde_json::from_slice(&bytes)
        .unwrap_or_else(|e| panic!("malformed golden JSON {}: {e}", path.display()))
}

/// Load a raw little-endian `f32` array fixture (e.g. `audio.f32`).
pub fn load_f32(rel: &str) -> Vec<f32> {
    let path = fixture_path(rel);
    let bytes = std::fs::read(&path)
        .unwrap_or_else(|e| panic!("missing f32 fixture {}: {e}", path.display()));
    assert!(
        bytes.len().is_multiple_of(4),
        "f32 fixture {} has length {} not divisible by 4",
        path.display(),
        bytes.len()
    );
    bytes
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

/// Exact JSON equality. Use for every deterministic pure-data layer.
#[track_caller]
pub fn assert_json_eq(actual: &Value, golden: &Value) {
    if actual != golden {
        panic!(
            "JSON parity mismatch\n  first diff: {}\n  actual: {}\n  golden: {}",
            first_json_diff(actual, golden).unwrap_or_else(|| "<root>".into()),
            actual,
            golden
        );
    }
}

/// Exact string equality (e.g. an emitted `.srt`/`.vtt` vs its golden).
#[track_caller]
pub fn assert_str_eq(actual: &str, golden: &str) {
    if actual != golden {
        let (line, a, g) = first_line_diff(actual, golden);
        panic!("string parity mismatch at line {line}\n  actual: {a:?}\n  golden: {g:?}");
    }
}

/// Element-wise `f32` closeness within `epsilon` (ulps-agnostic absolute test).
#[track_caller]
pub fn assert_f32_close(actual: &[f32], golden: &[f32], epsilon: f32) {
    assert_eq!(
        actual.len(),
        golden.len(),
        "f32 length mismatch: {} vs {}",
        actual.len(),
        golden.len()
    );
    for (i, (a, g)) in actual.iter().zip(golden).enumerate() {
        if !float_cmp::approx_eq!(f32, *a, *g, epsilon = epsilon) {
            panic!("f32[{i}] = {a} not within {epsilon} of golden {g}");
        }
    }
}

/// Tolerance for structural transcription parity (full pipeline only).
#[derive(Debug, Clone, Copy)]
pub struct SegTol {
    /// Allowed absolute difference in segment count.
    pub count: usize,
    /// Allowed per-segment start/end drift, in milliseconds.
    pub time_ms: u64,
    /// Minimum normalized token-set similarity for aligned segment text.
    pub text_ratio: f64,
}

impl Default for SegTol {
    fn default() -> Self {
        Self { count: 1, time_ms: 200, text_ratio: 0.9 }
    }
}

/// A minimal transcription segment view used for structural comparison.
#[derive(Debug, Clone)]
pub struct Seg {
    pub start: f64,
    pub end: f64,
    pub text: String,
}

/// Parse `[{start,end,text}, ...]` golden JSON into [`Seg`]s.
pub fn segs_from_json(v: &Value) -> Vec<Seg> {
    v.as_array()
        .unwrap_or_else(|| panic!("segments golden is not an array: {v}"))
        .iter()
        .map(|s| Seg {
            start: s["start"].as_f64().unwrap_or(0.0),
            end: s["end"].as_f64().unwrap_or(0.0),
            text: s["text"].as_str().unwrap_or("").to_string(),
        })
        .collect()
}

/// Structural transcription parity: count within tolerance, and each golden
/// segment has a positional counterpart whose timing and text are close.
#[track_caller]
pub fn assert_segments_close(actual: &[Seg], golden: &[Seg], tol: SegTol) {
    let diff = actual.len().abs_diff(golden.len());
    assert!(
        diff <= tol.count,
        "segment count {} differs from golden {} by {diff} > tol {}",
        actual.len(),
        golden.len(),
        tol.count
    );
    let n = actual.len().min(golden.len());
    for i in 0..n {
        let (a, g) = (&actual[i], &golden[i]);
        let ds = ((a.start - g.start).abs() * 1000.0) as u64;
        let de = ((a.end - g.end).abs() * 1000.0) as u64;
        assert!(
            ds <= tol.time_ms && de <= tol.time_ms,
            "segment {i} timing drift start={ds}ms end={de}ms > tol {}ms",
            tol.time_ms
        );
        let ratio = token_set_ratio(&a.text, &g.text);
        assert!(
            ratio >= tol.text_ratio,
            "segment {i} text ratio {ratio:.3} < {:.3}\n  actual: {:?}\n  golden: {:?}",
            tol.text_ratio,
            a.text,
            g.text
        );
    }
}

/// Process-global lock serializing env-mutating tests. The process environment
/// is shared mutable state, so parallel test threads must take turns on it.
static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// RAII environment isolation for the env-driven config parity tests.
///
/// Replaces `figment::Jail` in tests. `Jail::expect_with`'s closure is forced to
/// return `Result<(), figment::Error>`; that error is ~208 bytes and trips
/// `clippy::result_large_err`, which cannot be satisfied by boxing because the
/// type is fixed by figment's API. This guard delivers the same guarantees a
/// jail does — serialized execution (via [`ENV_LOCK`]), ambient `SUBMATE__*`
/// vars cleared so a developer/CI machine can't leak them into resolution, and
/// the previous environment restored on drop even if the test panics — without
/// routing through a large-error closure.
///
/// Hold the returned guard for the duration of the test:
/// `let _env = EnvGuard::set(&[("SUBMATE__SERVER__PORT", "9123")]);`
#[must_use = "the environment is restored when the guard is dropped; bind it for the test's scope"]
pub struct EnvGuard {
    _lock: std::sync::MutexGuard<'static, ()>,
    /// Keys touched, with their value before this guard (None = was unset), for
    /// exact restoration on drop.
    saved: Vec<(String, Option<String>)>,
}

impl EnvGuard {
    /// Acquire the global env lock, clear every ambient `SUBMATE__*` var, then
    /// apply `overrides`, recording prior values so [`Drop`] restores the exact
    /// previous state.
    pub fn set(overrides: &[(&str, &str)]) -> Self {
        // Recover from a poisoned lock (a prior test panicked mid-guard): the
        // data is just `()`, and Drop will have restored that test's env.
        let lock = ENV_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut saved: Vec<(String, Option<String>)> = Vec::new();

        // Snapshot and clear all ambient SUBMATE__* vars first.
        let ambient: Vec<String> = std::env::vars()
            .map(|(k, _)| k)
            .filter(|k| k.starts_with("SUBMATE__"))
            .collect();
        for key in ambient {
            saved.push((key.clone(), std::env::var(&key).ok()));
            // TODO: Audit that the environment access only happens in single-threaded code.
            unsafe { std::env::remove_var(&key) };
        }

        // Apply overrides, snapshotting any key not already recorded above.
        for (key, value) in overrides {
            if !saved.iter().any(|(k, _)| k == key) {
                saved.push(((*key).to_string(), std::env::var(key).ok()));
            }
            // TODO: Audit that the environment access only happens in single-threaded code.
            unsafe { std::env::set_var(key, value) };
        }

        Self { _lock: lock, saved }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (key, prior) in self.saved.drain(..) {
            match prior {
                // TODO: Audit that the environment access only happens in single-threaded code.
                Some(value) => unsafe { std::env::set_var(&key, value) },
                // TODO: Audit that the environment access only happens in single-threaded code.
                None => unsafe { std::env::remove_var(&key) },
            }
        }
    }
}

/// Normalized token-set similarity: |A∩B| / |A∪B| over lowercased,
/// punctuation-stripped whitespace tokens. 1.0 == identical token sets.
fn token_set_ratio(a: &str, b: &str) -> f64 {
    let toks = |s: &str| -> std::collections::BTreeSet<String> {
        s.split_whitespace()
            .map(|t| {
                t.chars()
                    .filter(|c| c.is_alphanumeric())
                    .flat_map(char::to_lowercase)
                    .collect::<String>()
            })
            .filter(|t| !t.is_empty())
            .collect()
    };
    let (sa, sb) = (toks(a), toks(b));
    if sa.is_empty() && sb.is_empty() {
        return 1.0;
    }
    let inter = sa.intersection(&sb).count() as f64;
    let union = sa.union(&sb).count() as f64;
    if union == 0.0 {
        1.0
    } else {
        inter / union
    }
}

/// Best-effort pointer to the first differing JSON path (object keys / array
/// indices), for readable failure messages. Returns `None` if equal.
fn first_json_diff(a: &Value, b: &Value) -> Option<String> {
    fn walk(a: &Value, b: &Value, path: &str) -> Option<String> {
        match (a, b) {
            (Value::Object(ma), Value::Object(mb)) => {
                for (k, va) in ma {
                    match mb.get(k) {
                        None => return Some(format!("{path}.{k} (missing in golden)")),
                        Some(vb) => {
                            if let Some(p) = walk(va, vb, &format!("{path}.{k}")) {
                                return Some(p);
                            }
                        }
                    }
                }
                for k in mb.keys() {
                    if !ma.contains_key(k) {
                        return Some(format!("{path}.{k} (missing in actual)"));
                    }
                }
                None
            }
            (Value::Array(aa), Value::Array(ab)) => {
                if aa.len() != ab.len() {
                    return Some(format!("{path}[] len {} vs {}", aa.len(), ab.len()));
                }
                for (i, (va, vb)) in aa.iter().zip(ab).enumerate() {
                    if let Some(p) = walk(va, vb, &format!("{path}[{i}]")) {
                        return Some(p);
                    }
                }
                None
            }
            _ => {
                if a == b {
                    None
                } else {
                    Some(path.to_string())
                }
            }
        }
    }
    walk(a, b, "$")
}

fn first_line_diff(a: &str, b: &str) -> (usize, String, String) {
    for (i, (la, lb)) in a.lines().zip(b.lines()).enumerate() {
        if la != lb {
            return (i + 1, la.to_string(), lb.to_string());
        }
    }
    let la = a.lines().count();
    let lb = b.lines().count();
    (la.min(lb) + 1, String::new(), String::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn json_eq_passes_and_diffs() {
        assert_json_eq(&json!({"a":1,"b":[1,2]}), &json!({"a":1,"b":[1,2]}));
        assert_eq!(
            first_json_diff(&json!({"a":1}), &json!({"a":2})),
            Some("$.a".to_string())
        );
    }

    #[test]
    fn f32_close_tolerance() {
        assert_f32_close(&[0.10000001], &[0.1], 1e-6);
    }

    #[test]
    fn segments_close_within_tolerance() {
        let a = vec![Seg { start: 0.0, end: 1.05, text: "Hello, world".into() }];
        let g = vec![Seg { start: 0.0, end: 1.0, text: "hello world!".into() }];
        assert_segments_close(&a, &g, SegTol::default());
    }

    #[test]
    fn token_set_ratio_is_order_insensitive() {
        assert!((token_set_ratio("a b c", "c b a") - 1.0).abs() < 1e-9);
        assert!(token_set_ratio("a b", "a b c d") < 1.0);
    }
}
