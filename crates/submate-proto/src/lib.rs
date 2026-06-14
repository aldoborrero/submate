//! Wire types shared by the server and processing nodes.
//!
//! This crate defines the JSON contract for the node-coordination API described
//! in `docs/architecture.md` (FileFlows/Unmanic-style topology, no external
//! broker): a node registers its capabilities, long-polls for work, fetches the
//! extracted PCM by URL, then reports progress and a final result.
//!
//! The crate is **pure** — `serde` only, no I/O. Shared domain enums are reused
//! from `submate-types` so the wire vocabulary (transcribe/translate, whisper
//! model, device) matches the rest of the system byte-for-byte. Language fields
//! cross the wire as ISO-639 strings (the same form the audio pipeline consumes),
//! not as the `submate-lang` enum, keeping this crate dependency-light.
//!
//! Every message round-trips through serde (serialize → deserialize → equal);
//! see the `tests` module.

use serde::{Deserialize, Serialize};
use submate_types::{Device, TranscriptionTask, TranslationBackend, WhisperModel};

/// Subtitle output format a node should emit for a job.
///
/// Carried on [`JobOpts`] so both the synchronous and queued paths render the
/// format the user picked at the CLI. The wire strings are the bare format
/// names (`"srt"`, `"vtt"`, …); the field serde-defaults to [`OutputFormat::Srt`]
/// so older payloads (and the queued path before this field existed) keep
/// producing SRT.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    /// SubRip subtitles (`.srt`).
    #[default]
    Srt,
    /// WebVTT subtitles (`.vtt`).
    Vtt,
    /// Advanced SubStation Alpha subtitles (`.ass`).
    Ass,
    /// JSON dump of the full transcription result (`.json`).
    Json,
    /// Plain-text transcript, no timestamps (`.txt`).
    Txt,
}

impl OutputFormat {
    /// File extension including the leading dot (e.g. `".srt"`).
    pub fn extension(self) -> &'static str {
        match self {
            Self::Srt => ".srt",
            Self::Vtt => ".vtt",
            Self::Ass => ".ass",
            Self::Json => ".json",
            Self::Txt => ".txt",
        }
    }
}

/// Node → server: announce capabilities and claim a coordination token.
///
/// Mirrors `POST /nodes/register` (`{id, gpu, runners, tasks}`). `runners` is the
/// per-node concurrency the dispatcher will honour; `tasks` is the set of job
/// kinds this node is willing to run (a translation-only CPU node advertises just
/// `Translate`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeRegister {
    /// Stable node identifier (chosen by the node, unique per server).
    pub id: String,
    /// Whether the node has a usable GPU (gates GPU-only job routing).
    pub gpu: bool,
    /// Maximum concurrent jobs the node's dispatcher will run.
    pub runners: u32,
    /// Job kinds this node is willing to accept.
    pub tasks: Vec<TranscriptionTask>,
}

/// Server → node: token issued in response to [`NodeRegister`].
///
/// Subsequent `request-work`/`progress`/`result`/`heartbeat` calls authenticate
/// with this token.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeRegistered {
    /// Opaque bearer token scoped to the registered node.
    pub token: String,
}

/// Node → server: long-poll for the next claimable job.
///
/// Mirrors `POST /nodes/{id}/request-work`. The server runs the atomic,
/// capability-filtered claim and replies with a [`WorkResponse`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkRequest {
    /// The node asking for work (must match a prior registration).
    pub node_id: String,
}

/// Server → node: the claimed job, or an explicit "nothing to do".
///
/// Serialized as an internally tagged enum so an empty poll is still a
/// well-typed JSON body (`{"type":"no_work"}`) rather than an absent payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WorkResponse {
    /// A job was claimed for this node.
    Work {
        /// Server-side job identifier; echoed back on progress/result.
        job_id: String,
        /// Transcribe vs. translate (reuses the shared domain enum).
        kind: TranscriptionTask,
        /// URL the node `GET`s to pull the extracted PCM payload.
        ///
        /// Large audio is fetched, never inlined in this JSON (see
        /// architecture.md "Job payload & audio transfer").
        audio_url: String,
        /// Job parameters (model, language, translation backend, …).
        opts: JobOpts,
    },
    /// The long-poll completed with no claimable job.
    NoWork,
}

/// Parameters that travel with a [`WorkResponse::Work`] job.
///
/// Languages are ISO-639 strings (e.g. `"en"`), matching what the Whisper and
/// translation pipelines consume; `None` means "auto-detect" / "not applicable".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobOpts {
    /// Whisper model size to load for this job.
    pub model: WhisperModel,
    /// Device the node should run on (`auto` lets the node decide).
    pub device: Device,
    /// Source-audio language hint, or `None` to auto-detect.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_language: Option<String>,
    /// Target language for `translate` jobs, or `None` for transcribe-as-is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_language: Option<String>,
    /// LLM backend for translation jobs; `None` for plain transcription.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub translation_backend: Option<TranslationBackend>,
    /// Subtitle format the node should emit; defaults to [`OutputFormat::Srt`]
    /// so payloads without this field keep producing SRT.
    #[serde(default)]
    pub output_format: OutputFormat,
}

/// Node → server: in-flight progress for a running job.
///
/// Mirrors `POST /jobs/{id}/progress`. `pct` is a fraction in `[0.0, 1.0]`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Progress {
    /// Job this progress update is for.
    pub job_id: String,
    /// Completion fraction in `[0.0, 1.0]`.
    pub pct: f32,
}

/// Node → server: terminal result for a job.
///
/// Mirrors `POST /jobs/{id}/result`. The boolean `ok` discriminates the
/// [`outcome`](JobResult::outcome): success carries `output` (the produced
/// subtitle text), failure carries `error` (the message). The wire form is
/// `{"job_id":…,"ok":true,"output":…}` / `{"job_id":…,"ok":false,"error":…}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobResult {
    /// Job this result is for.
    pub job_id: String,
    /// Whether the job succeeded, and the matching payload.
    #[serde(flatten)]
    pub outcome: JobOutcome,
}

/// Success/failure payload of a [`JobResult`].
///
/// Externally discriminated by the boolean `ok` field, so success and failure
/// can never both be present and the reader checks one flag.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "JobOutcomeWire", into = "JobOutcomeWire")]
pub enum JobOutcome {
    /// Job succeeded; `output` is the produced subtitle text.
    Ok {
        /// Produced subtitle text (e.g. SRT content).
        output: String,
    },
    /// Job failed; `error` is the failure message.
    Err {
        /// Human-readable failure reason.
        error: String,
    },
}

/// Flat wire representation of [`JobOutcome`]: a real boolean `ok` plus the
/// matching optional payload. `JobOutcome` converts to/from this so the public
/// API stays a sum type while the JSON stays `{ok, output|error}`.
#[derive(Serialize, Deserialize)]
struct JobOutcomeWire {
    ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    output: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

impl From<JobOutcome> for JobOutcomeWire {
    fn from(o: JobOutcome) -> Self {
        match o {
            JobOutcome::Ok { output } => Self {
                ok: true,
                output: Some(output),
                error: None,
            },
            JobOutcome::Err { error } => Self {
                ok: false,
                output: None,
                error: Some(error),
            },
        }
    }
}

impl From<JobOutcomeWire> for JobOutcome {
    fn from(w: JobOutcomeWire) -> Self {
        if w.ok {
            Self::Ok {
                output: w.output.unwrap_or_default(),
            }
        } else {
            Self::Err {
                error: w.error.unwrap_or_default(),
            }
        }
    }
}

/// Node → server: keep-alive to extend the job lease.
///
/// Mirrors `POST /nodes/{id}/heartbeat`. A node that stops heartbeating has its
/// claimed jobs reclaimed (`locked_at + lease < now` → `queued`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Heartbeat {
    /// Node sending the keep-alive.
    pub node_id: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Serialize → deserialize → equal for any wire message.
    fn assert_roundtrip<T>(value: &T)
    where
        T: Serialize + for<'de> Deserialize<'de> + PartialEq + std::fmt::Debug,
    {
        let json = serde_json::to_string(value).expect("serialize");
        let back: T = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(value, &back, "round-trip mismatch via {json}");
    }

    #[test]
    fn roundtrip_node_register() {
        assert_roundtrip(&NodeRegister {
            id: "gpu-box-1".into(),
            gpu: true,
            runners: 4,
            tasks: vec![TranscriptionTask::Transcribe, TranscriptionTask::Translate],
        });
    }

    #[test]
    fn roundtrip_node_registered() {
        assert_roundtrip(&NodeRegistered {
            token: "tok_abc123".into(),
        });
    }

    #[test]
    fn roundtrip_work_request() {
        assert_roundtrip(&WorkRequest {
            node_id: "gpu-box-1".into(),
        });
    }

    #[test]
    fn roundtrip_work_response_work() {
        assert_roundtrip(&WorkResponse::Work {
            job_id: "job-42".into(),
            kind: TranscriptionTask::Translate,
            audio_url: "/jobs/job-42/audio".into(),
            opts: JobOpts {
                model: WhisperModel::LargeV3,
                device: Device::Cuda,
                source_language: Some("ja".into()),
                target_language: Some("en".into()),
                translation_backend: Some(TranslationBackend::Claude),
                output_format: OutputFormat::Ass,
            },
        });
    }

    #[test]
    fn roundtrip_work_response_no_work() {
        assert_roundtrip(&WorkResponse::NoWork);
    }

    #[test]
    fn roundtrip_job_opts_minimal() {
        assert_roundtrip(&JobOpts {
            model: WhisperModel::Medium,
            device: Device::Auto,
            source_language: None,
            target_language: None,
            translation_backend: None,
            output_format: OutputFormat::default(),
        });
    }

    #[test]
    fn roundtrip_progress() {
        assert_roundtrip(&Progress {
            job_id: "job-42".into(),
            pct: 0.5,
        });
    }

    #[test]
    fn roundtrip_job_result_ok() {
        assert_roundtrip(&JobResult {
            job_id: "job-42".into(),
            outcome: JobOutcome::Ok {
                output: "1\n00:00:00,000 --> 00:00:01,000\nhi\n".into(),
            },
        });
    }

    #[test]
    fn roundtrip_job_result_err() {
        assert_roundtrip(&JobResult {
            job_id: "job-42".into(),
            outcome: JobOutcome::Err {
                error: "model load failed".into(),
            },
        });
    }

    #[test]
    fn roundtrip_heartbeat() {
        assert_roundtrip(&Heartbeat {
            node_id: "gpu-box-1".into(),
        });
    }

    /// The empty poll serializes to a self-describing tagged object, not an
    /// absent body — so callers can distinguish "no work" from a transport error.
    #[test]
    fn no_work_is_tagged_object() {
        let json = serde_json::to_string(&WorkResponse::NoWork).unwrap();
        assert_eq!(json, r#"{"type":"no_work"}"#);
    }

    /// `JobResult` flattens success onto a real boolean `ok` with `output`.
    #[test]
    fn job_result_ok_wire_shape() {
        let json = serde_json::to_string(&JobResult {
            job_id: "j".into(),
            outcome: JobOutcome::Ok { output: "x".into() },
        })
        .unwrap();
        assert_eq!(json, r#"{"job_id":"j","ok":true,"output":"x"}"#);
    }

    /// A `JobOpts` payload without `output_format` deserializes to the SRT
    /// default, so pre-field producers (and the queued path) keep emitting SRT.
    #[test]
    fn job_opts_output_format_defaults_to_srt() {
        let opts: JobOpts =
            serde_json::from_str(r#"{"model":"medium","device":"auto"}"#).expect("deserialize");
        assert_eq!(opts.output_format, OutputFormat::Srt);
    }

    /// Each `OutputFormat` maps to its dotted file extension.
    #[test]
    fn output_format_extension() {
        assert_eq!(OutputFormat::Srt.extension(), ".srt");
        assert_eq!(OutputFormat::Vtt.extension(), ".vtt");
        assert_eq!(OutputFormat::Ass.extension(), ".ass");
        assert_eq!(OutputFormat::Json.extension(), ".json");
        assert_eq!(OutputFormat::Txt.extension(), ".txt");
    }

    /// `JobResult` failure carries `ok:false` with `error`.
    #[test]
    fn job_result_err_wire_shape() {
        let json = serde_json::to_string(&JobResult {
            job_id: "j".into(),
            outcome: JobOutcome::Err {
                error: "boom".into(),
            },
        })
        .unwrap();
        assert_eq!(json, r#"{"job_id":"j","ok":false,"error":"boom"}"#);
    }
}
