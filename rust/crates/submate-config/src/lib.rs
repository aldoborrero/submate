//! Layered configuration via figment (ports `submate/config.py`).
//!
//! This crate defines the settings structs that mirror the Pydantic
//! `BaseModel`/`BaseSettings` classes (each field carrying a
//! `#[serde(default = ...)]` whose value matches the Python default
//! byte-for-byte) *and* the figment provider chain that resolves them from the
//! `SUBMATE__` environment and an optional `--config-file`.
//!
//! Enums are reused from [`submate_types`] rather than redefined, so their
//! string forms stay in lockstep with the rest of the port.
//!
//! # Precedence
//!
//! Mirrors Pydantic-Settings' source order (`settings_customise_sources` in
//! `submate/config.py`): environment variables win over the config file, which
//! wins over the built-in defaults. In figment terms the chain merges, in
//! order, `Serialized::defaults(Config::default())`, then the `--config-file`
//! JSON (if any), then `Env::prefixed("SUBMATE__").split("__")` — figment's
//! merge lets later providers override earlier ones.
//!
//! Nested settings use the `__` delimiter, e.g. `SUBMATE__WHISPER__MODEL` maps
//! to `whisper.model`, exactly as the Python `env_nested_delimiter="__"`.
//!
//! # Parity
//!
//! * `tests/parity.rs` falsifier `parity::defaults` serializes a
//!   default-constructed [`Config`] and diffs it against
//!   `rust/fixtures/config/defaults.resolved.json`.
//! * `tests/parity.rs` falsifier `parity::env_nesting` loads
//!   `rust/fixtures/config/nested.env` through [`Config::from_env`] and diffs
//!   the result against `rust/fixtures/config/nested.resolved.json`.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use submate_types::{Device, LanguageNamingType, TranslationBackend, WhisperImplementation};

use std::path::Path;

use figment::{
    providers::{Env, Format, Json, Serialized},
    Figment,
};

/// A field that is either a string or a bool (`str | bool` in Python).
///
/// Used by [`StableTsSettings::custom_regroup`]: a regroup pattern string, or
/// `false` to disable. `#[serde(untagged)]` serializes the inner value
/// directly, so the JSON form is a bare string or bare bool — matching Python.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum StrOrBool {
    /// A regroup pattern string.
    Str(String),
    /// `false` disables regrouping.
    Bool(bool),
}

/// Whisper model and transcription settings (`WhisperSettings`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WhisperSettings {
    #[serde(default = "default_whisper_model")]
    pub model: String,
    #[serde(default = "default_device")]
    pub device: Device,
    #[serde(default = "default_compute_type")]
    pub compute_type: String,
    #[serde(default = "default_implementation")]
    pub implementation: WhisperImplementation,
    #[serde(default)]
    pub transcribe_kwargs: Map<String, Value>,
    #[serde(default)]
    pub folders: Vec<String>,
}

fn default_whisper_model() -> String {
    "medium".to_string()
}
fn default_device() -> Device {
    Device::Cpu
}
fn default_compute_type() -> String {
    "int8".to_string()
}
fn default_implementation() -> WhisperImplementation {
    WhisperImplementation::FasterWhisper
}

impl Default for WhisperSettings {
    fn default() -> Self {
        WhisperSettings {
            model: default_whisper_model(),
            device: default_device(),
            compute_type: default_compute_type(),
            implementation: default_implementation(),
            transcribe_kwargs: Map::new(),
            folders: Vec::new(),
        }
    }
}

/// Stable-ts subtitle generation settings (`StableTsSettings`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StableTsSettings {
    #[serde(default)]
    pub word_level_highlight: bool,
    #[serde(default = "default_custom_regroup")]
    pub custom_regroup: StrOrBool,
    #[serde(default = "default_true")]
    pub suppress_silence: bool,
    #[serde(default = "default_min_word_duration")]
    pub min_word_duration: f64,
}

fn default_custom_regroup() -> StrOrBool {
    StrOrBool::Str("cm_sl=84_sl=42++++++1".to_string())
}
fn default_min_word_duration() -> f64 {
    0.1
}

impl Default for StableTsSettings {
    fn default() -> Self {
        StableTsSettings {
            word_level_highlight: false,
            custom_regroup: default_custom_regroup(),
            suppress_silence: true,
            min_word_duration: default_min_word_duration(),
        }
    }
}

/// Server and processing settings (`ServerSettings`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ServerSettings {
    #[serde(default = "default_address")]
    pub address: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_concurrent_transcriptions")]
    pub concurrent_transcriptions: u32,
    #[serde(default = "default_true")]
    pub process_on_add: bool,
    #[serde(default)]
    pub process_on_play: bool,
    #[serde(default = "default_true")]
    pub bazarr_enabled: bool,
    #[serde(default = "default_true")]
    pub jellyfin_enabled: bool,
    #[serde(default = "default_true")]
    pub status_enabled: bool,
    #[serde(default = "default_true")]
    pub bazarr_keep_model_loaded: bool,
    #[serde(default = "default_bazarr_model_idle_timeout")]
    pub bazarr_model_idle_timeout: u32,
}

fn default_address() -> String {
    "0.0.0.0".to_string()
}
fn default_port() -> u16 {
    9000
}
fn default_concurrent_transcriptions() -> u32 {
    2
}
fn default_bazarr_model_idle_timeout() -> u32 {
    300
}

impl Default for ServerSettings {
    fn default() -> Self {
        ServerSettings {
            address: default_address(),
            port: default_port(),
            concurrent_transcriptions: default_concurrent_transcriptions(),
            process_on_add: true,
            process_on_play: false,
            bazarr_enabled: true,
            jellyfin_enabled: true,
            status_enabled: true,
            bazarr_keep_model_loaded: true,
            bazarr_model_idle_timeout: default_bazarr_model_idle_timeout(),
        }
    }
}

/// Path mapping settings for Docker deployments (`PathMappingSettings`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct PathMappingSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub from_path: String,
    #[serde(default)]
    pub to_path: String,
}

/// Jellyfin media server integration settings (`JellyfinSettings`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct JellyfinSettings {
    #[serde(default)]
    pub server_url: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub libraries: Vec<String>,
}

/// Queue and retry settings (`QueueSettings`).
///
/// The Python default for `db_path` is empty, with a root `model_validator`
/// later filling in `{XDG_DATA_HOME}/subgen/queue.db`. The captured golden
/// records the resolved-but-unexpanded form `${XDG_DATA_HOME}/subgen/queue.db`;
/// the actual expansion is part of the downstream resolution item.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QueueSettings {
    #[serde(default = "default_db_path")]
    pub db_path: String,
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
    #[serde(default = "default_retry_delay")]
    pub retry_delay: u32,
}

fn default_db_path() -> String {
    "${XDG_DATA_HOME}/subgen/queue.db".to_string()
}
fn default_max_retries() -> u32 {
    3
}
fn default_retry_delay() -> u32 {
    5
}

impl Default for QueueSettings {
    fn default() -> Self {
        QueueSettings {
            db_path: default_db_path(),
            max_retries: default_max_retries(),
            retry_delay: default_retry_delay(),
        }
    }
}

/// Subtitle generation and language settings (`SubtitleSettings`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SubtitleSettings {
    #[serde(default)]
    pub force_detected_language_to: String,
    #[serde(default)]
    pub append_credits: bool,
    #[serde(default = "default_true")]
    pub skip_if_target_subtitle_exists: bool,
    #[serde(default)]
    pub skip_if_external_subtitles_exist: bool,
    #[serde(default)]
    pub skip_if_internal_subtitle_language: String,
    #[serde(default)]
    pub skip_subtitle_languages: Vec<String>,
    #[serde(default)]
    pub skip_if_audio_languages: Vec<String>,
    #[serde(default)]
    pub skip_unknown_language: bool,
    #[serde(default)]
    pub preferred_audio_languages: Vec<String>,
    #[serde(default)]
    pub limit_to_preferred_audio_languages: bool,
    #[serde(default = "default_true")]
    pub lrc_for_audio_files: bool,
    #[serde(default)]
    pub only_skip_if_subgen_subtitle: bool,
    #[serde(default)]
    pub skip_if_no_language_but_subtitles_exist: bool,
    #[serde(default = "default_language_naming_type")]
    pub language_naming_type: LanguageNamingType,
    #[serde(default)]
    pub include_subgen_marker: bool,
    #[serde(default)]
    pub include_model_in_filename: bool,
}

fn default_language_naming_type() -> LanguageNamingType {
    LanguageNamingType::Iso6392B
}

impl Default for SubtitleSettings {
    fn default() -> Self {
        SubtitleSettings {
            force_detected_language_to: String::new(),
            append_credits: false,
            skip_if_target_subtitle_exists: true,
            skip_if_external_subtitles_exist: false,
            skip_if_internal_subtitle_language: String::new(),
            skip_subtitle_languages: Vec::new(),
            skip_if_audio_languages: Vec::new(),
            skip_unknown_language: false,
            preferred_audio_languages: Vec::new(),
            limit_to_preferred_audio_languages: false,
            lrc_for_audio_files: true,
            only_skip_if_subgen_subtitle: false,
            skip_if_no_language_but_subtitles_exist: false,
            language_naming_type: default_language_naming_type(),
            include_subgen_marker: false,
            include_model_in_filename: false,
        }
    }
}

/// Translation settings for LLM-backed subtitle translation (`TranslationSettings`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TranslationSettings {
    #[serde(default = "default_backend")]
    pub backend: TranslationBackend,
    #[serde(default = "default_ollama_model")]
    pub ollama_model: String,
    #[serde(default = "default_ollama_url")]
    pub ollama_url: String,
    #[serde(default)]
    pub anthropic_api_key: String,
    #[serde(default = "default_claude_model")]
    pub claude_model: String,
    #[serde(default)]
    pub openai_api_key: String,
    #[serde(default = "default_openai_model")]
    pub openai_model: String,
    #[serde(default)]
    pub gemini_api_key: String,
    #[serde(default = "default_gemini_model")]
    pub gemini_model: String,
    #[serde(default = "default_chunk_size")]
    pub chunk_size: u32,
}

fn default_backend() -> TranslationBackend {
    TranslationBackend::Ollama
}
fn default_ollama_model() -> String {
    "llama3.2".to_string()
}
fn default_ollama_url() -> String {
    "http://localhost:11434".to_string()
}
fn default_claude_model() -> String {
    "claude-sonnet-4-20250514".to_string()
}
fn default_openai_model() -> String {
    "gpt-4o-mini".to_string()
}
fn default_gemini_model() -> String {
    "gemini-2.0-flash".to_string()
}
fn default_chunk_size() -> u32 {
    50
}

impl Default for TranslationSettings {
    fn default() -> Self {
        TranslationSettings {
            backend: default_backend(),
            ollama_model: default_ollama_model(),
            ollama_url: default_ollama_url(),
            anthropic_api_key: String::new(),
            claude_model: default_claude_model(),
            openai_api_key: String::new(),
            openai_model: default_openai_model(),
            gemini_api_key: String::new(),
            gemini_model: default_gemini_model(),
            chunk_size: default_chunk_size(),
        }
    }
}

/// Root application configuration (`Config`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub whisper: WhisperSettings,
    #[serde(default)]
    pub stable_ts: StableTsSettings,
    #[serde(default)]
    pub server: ServerSettings,
    #[serde(default)]
    pub path_mapping: PathMappingSettings,
    #[serde(default)]
    pub jellyfin: JellyfinSettings,
    #[serde(default)]
    pub queue: QueueSettings,
    #[serde(default)]
    pub subtitle: SubtitleSettings,
    #[serde(default)]
    pub translation: TranslationSettings,
    #[serde(default)]
    pub debug: bool,
    #[serde(default)]
    pub clear_vram_on_complete: bool,
}

impl Config {
    /// Build the figment provider chain: defaults < config file < env.
    ///
    /// Kept separate from [`Config::from_env`] so resolution can be exercised
    /// without extracting, and so the chain order lives in one place.
    fn figment(config_file: Option<&Path>) -> Figment {
        let mut figment = Figment::from(Serialized::defaults(Config::default()));

        // Optional `--config-file` JSON layer. Ports `get_config(config_file)`:
        // a file supplies overrides on top of the defaults, but the env still
        // wins. `Json::file` is a no-op if the path is absent, so a missing
        // file simply contributes nothing rather than erroring.
        if let Some(path) = config_file {
            figment = figment.merge(Json::file(path));
        }

        // `SUBMATE__WHISPER__MODEL` -> `whisper.model`. `split("__")` turns the
        // nested delimiter into figment key-path components, matching Pydantic's
        // `env_nested_delimiter="__"`. Env is merged last, so it has the final
        // say — Pydantic's env-over-file precedence.
        figment.merge(Env::prefixed("SUBMATE__").split("__"))
    }

    /// Resolve configuration from the `SUBMATE__` environment plus an optional
    /// `--config-file` JSON path.
    ///
    /// Ports `submate.config.get_config`. Precedence is env > file > defaults
    /// (see the module-level docs). Returns a figment error if a value fails to
    /// coerce into its field type (e.g. a non-numeric `SUBMATE__SERVER__PORT`).
    ///
    /// The error is boxed: `figment::Error` is a large enum, so an unboxed
    /// `Result` would bloat every caller's stack frame on the happy path.
    pub fn from_env(config_file: Option<&Path>) -> Result<Config, Box<figment::Error>> {
        Self::figment(config_file).extract().map_err(Box::new)
    }

    /// Convenience entrypoint equivalent to `from_env(None)`: resolve purely
    /// from defaults and the `SUBMATE__` environment.
    pub fn load() -> Result<Config, Box<figment::Error>> {
        Self::from_env(None)
    }
}

/// `serde` default helper for `bool` fields that default to `true`.
fn default_true() -> bool {
    true
}
