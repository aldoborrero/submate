//! Layered configuration via figment.
//!
//! This crate defines the settings structs (each struct's `impl Default`
//! carrying the defaults) *and* the figment provider chain that resolves
//! them from the `SUBMATE__` environment and an optional `--config-file`.
//!
//! Defaults live in one place per struct: the `impl Default`. A container-level
//! `#[serde(default)]` makes a missing field fall back to that `Default`, so the
//! defaults are never restated as per-field serde attributes.
//!
//! Enums are reused from [`submate_types`] rather than redefined, so their
//! string forms stay in lockstep with the rest of the crates.
//!
//! # Precedence
//!
//! Source order: environment variables win over the config file, which
//! wins over the built-in defaults. In figment terms the chain merges, in
//! order, `Serialized::defaults(Config::default())`, then the `--config-file`
//! JSON (if any), then `Env::prefixed("SUBMATE__").split("__")` — figment's
//! merge lets later providers override earlier ones.
//!
//! Nested settings use the `__` delimiter, e.g. `SUBMATE__WHISPER__MODEL` maps
//! to `whisper.model`.
//!
//! # Parity
//!
//! * `tests/parity.rs` falsifier `parity::defaults` serializes a
//!   default-constructed [`Config`] and diffs it against
//!   `fixtures/config/defaults.resolved.json`.
//! * `tests/parity.rs` falsifier `parity::env_nesting` loads
//!   `fixtures/config/nested.env` through [`Config::from_env`] and diffs
//!   the result against `fixtures/config/nested.resolved.json`.

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use submate_types::{Device, TranslationBackend};

use std::path::Path;

use figment::{
    Figment,
    providers::{Env, Format, Json, Serialized},
};

/// A field that is either a string or a bool.
///
/// Used by [`StableTsSettings::custom_regroup`]: a regroup pattern string, or
/// `false` to disable. `#[serde(untagged)]` serializes the inner value
/// directly, so the JSON form is a bare string or bare bool.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum StrOrBool {
    /// A regroup pattern string.
    Str(String),
    /// `false` disables regrouping.
    Bool(bool),
}

/// Whisper model and transcription settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WhisperSettings {
    pub model: String,
    pub device: Device,

    // whisper.cpp decoding knobs. `None` leaves whisper.cpp's own default; each
    // is also exposed as a CLI flag on `transcribe` (`--initial-prompt`,
    // `--beam-size`, …) and via `SUBMATE__WHISPER__*`. Skipped from the
    // serialized form when unset so they stay out of `config show` until used.
    /// Prompt text that biases the decoder's vocabulary/spelling.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initial_prompt: Option<String>,
    /// Beam-search width; unset uses greedy decoding.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub beam_size: Option<u32>,
    /// Sampling temperature.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    /// No-speech probability above which a segment is treated as silence.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub no_speech_threshold: Option<f32>,
    /// Entropy threshold for the decoder's temperature fallback.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entropy_threshold: Option<f32>,
    /// Average-log-probability threshold below which a decode is rejected.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logprob_threshold: Option<f32>,
    /// Maximum characters per segment (caps subtitle line length).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_len: Option<u32>,
}

impl Default for WhisperSettings {
    fn default() -> Self {
        Self {
            model: "medium".to_string(),
            device: Device::Cpu,
            initial_prompt: None,
            beam_size: None,
            temperature: None,
            no_speech_threshold: None,
            entropy_threshold: None,
            logprob_threshold: None,
            max_len: None,
        }
    }
}

/// Stable-ts subtitle generation settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StableTsSettings {
    pub word_level_highlight: bool,
    #[serde(deserialize_with = "deserialize_regroup")]
    pub custom_regroup: StrOrBool,
    pub suppress_silence: bool,
    pub min_word_duration: f64,
}

impl Default for StableTsSettings {
    fn default() -> Self {
        Self {
            word_level_highlight: false,
            custom_regroup: StrOrBool::Str("cm_sl=84_sl=42++++++1".to_string()),
            suppress_silence: true,
            min_word_duration: 0.1,
        }
    }
}

/// Server and processing settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ServerSettings {
    pub address: String,
    pub port: u16,
    pub concurrent_transcriptions: u32,
}

impl Default for ServerSettings {
    fn default() -> Self {
        Self {
            address: "0.0.0.0".to_string(),
            port: 9000,
            concurrent_transcriptions: 2,
        }
    }
}

/// Translation settings for LLM-backed subtitle translation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TranslationSettings {
    pub backend: TranslationBackend,
    pub ollama_model: String,
    pub ollama_url: String,
    pub anthropic_api_key: String,
    pub claude_model: String,
    pub openai_api_key: String,
    pub openai_model: String,
    pub gemini_api_key: String,
    pub gemini_model: String,
    pub chunk_size: u32,
}

impl Default for TranslationSettings {
    fn default() -> Self {
        Self {
            backend: TranslationBackend::Ollama,
            ollama_model: "llama3.2".to_string(),
            ollama_url: "http://localhost:11434".to_string(),
            anthropic_api_key: String::new(),
            claude_model: "claude-sonnet-4-6".to_string(),
            openai_api_key: String::new(),
            openai_model: "gpt-5-mini".to_string(),
            gemini_api_key: String::new(),
            gemini_model: "gemini-2.5-flash".to_string(),
            chunk_size: 50,
        }
    }
}

/// Root application configuration.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub whisper: WhisperSettings,
    pub stable_ts: StableTsSettings,
    pub server: ServerSettings,
    pub translation: TranslationSettings,
    pub debug: bool,
}

impl Config {
    /// Build the figment provider chain: defaults < config file < env.
    ///
    /// Kept separate from [`Config::from_env`] so resolution can be exercised
    /// without extracting, and so the chain order lives in one place.
    fn figment(config_file: Option<&Path>) -> Figment {
        let mut figment = Figment::from(Serialized::defaults(Self::default()));

        // Optional `--config-file` JSON layer: a file supplies overrides on top
        // of the defaults, but the env still wins. `Json::file` is a no-op if
        // the path is absent, so a missing file simply contributes nothing
        // rather than erroring.
        if let Some(path) = config_file {
            figment = figment.merge(Json::file(path));
        }

        // `SUBMATE__WHISPER__MODEL` -> `whisper.model`. `split("__")` turns the
        // nested delimiter into figment key-path components. Env is merged last,
        // so it has the final say — env-over-file precedence.
        figment.merge(Env::prefixed("SUBMATE__").split("__"))
    }

    /// Resolve configuration from the `SUBMATE__` environment plus an optional
    /// `--config-file` JSON path.
    ///
    /// Precedence is env > file > defaults (see the module-level docs). Returns
    /// a figment error if a value fails to coerce into its field type (e.g. a
    /// non-numeric `SUBMATE__SERVER__PORT`).
    ///
    /// The error is boxed: `figment::Error` is a large enum, so an unboxed
    /// `Result` would bloat every caller's stack frame on the happy path.
    pub fn from_env(config_file: Option<&Path>) -> Result<Self, Box<figment::Error>> {
        Self::figment(config_file).extract().map_err(Box::new)
    }

    /// Convenience entrypoint equivalent to `from_env(None)`: resolve purely
    /// from defaults and the `SUBMATE__` environment.
    pub fn load() -> Result<Self, Box<figment::Error>> {
        Self::from_env(None)
    }
}

/// Coerce a regroup env value into a [`StrOrBool`], or pass through an
/// already-typed bool/string.
///
/// A string in `{false, off, 0, no, ""}` (case-insensitive) disables regrouping
/// (`Bool(false)`); any other string is a regroup pattern (`Str(_)`). A real
/// bool from the file/defaults layer passes through unchanged.
fn deserialize_regroup<'de, D>(deserializer: D) -> Result<StrOrBool, D::Error>
where
    D: Deserializer<'de>,
{
    match Value::deserialize(deserializer)? {
        Value::Bool(b) => Ok(StrOrBool::Bool(b)),
        Value::String(s) => {
            if matches!(s.to_lowercase().as_str(), "false" | "off" | "0" | "no" | "") {
                Ok(StrOrBool::Bool(false))
            } else {
                Ok(StrOrBool::Str(s))
            }
        }
        other => Err(serde::de::Error::custom(format!(
            "custom_regroup must be a string or bool, got {other}"
        ))),
    }
}
