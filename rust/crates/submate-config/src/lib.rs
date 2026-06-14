//! Layered configuration via figment (ports `submate/config.py`).
//!
//! This crate defines the settings structs that mirror the Pydantic
//! `BaseModel`/`BaseSettings` classes (each struct's `impl Default` carrying the
//! Python defaults byte-for-byte) *and* the figment provider chain that resolves
//! them from the `SUBMATE__` environment and an optional `--config-file`.
//!
//! Defaults live in one place per struct: the `impl Default`. A container-level
//! `#[serde(default)]` makes a missing field fall back to that `Default`, so the
//! defaults are never restated as per-field serde attributes.
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

use serde::{Deserialize, Deserializer, Serialize};
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
#[serde(default)]
pub struct WhisperSettings {
    pub model: String,
    pub device: Device,
    pub compute_type: String,
    pub implementation: WhisperImplementation,
    #[serde(deserialize_with = "deserialize_json_kwargs")]
    pub transcribe_kwargs: Map<String, Value>,
    #[serde(deserialize_with = "deserialize_pipe_list")]
    pub folders: Vec<String>,
}

impl Default for WhisperSettings {
    fn default() -> Self {
        WhisperSettings {
            model: "medium".to_string(),
            device: Device::Cpu,
            compute_type: "int8".to_string(),
            implementation: WhisperImplementation::FasterWhisper,
            transcribe_kwargs: Map::new(),
            folders: Vec::new(),
        }
    }
}

/// Stable-ts subtitle generation settings (`StableTsSettings`).
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
        StableTsSettings {
            word_level_highlight: false,
            custom_regroup: StrOrBool::Str("cm_sl=84_sl=42++++++1".to_string()),
            suppress_silence: true,
            min_word_duration: 0.1,
        }
    }
}

/// Server and processing settings (`ServerSettings`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ServerSettings {
    pub address: String,
    pub port: u16,
    pub concurrent_transcriptions: u32,
    pub process_on_add: bool,
    pub process_on_play: bool,
    pub bazarr_enabled: bool,
    pub jellyfin_enabled: bool,
    pub status_enabled: bool,
    pub bazarr_keep_model_loaded: bool,
    pub bazarr_model_idle_timeout: u32,
}

impl Default for ServerSettings {
    fn default() -> Self {
        ServerSettings {
            address: "0.0.0.0".to_string(),
            port: 9000,
            concurrent_transcriptions: 2,
            process_on_add: true,
            process_on_play: false,
            bazarr_enabled: true,
            jellyfin_enabled: true,
            status_enabled: true,
            bazarr_keep_model_loaded: true,
            bazarr_model_idle_timeout: 300,
        }
    }
}

/// Path mapping settings for Docker deployments (`PathMappingSettings`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PathMappingSettings {
    pub enabled: bool,
    pub from_path: String,
    pub to_path: String,
}

/// Jellyfin media server integration settings (`JellyfinSettings`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct JellyfinSettings {
    pub server_url: String,
    pub api_key: String,
    #[serde(deserialize_with = "deserialize_pipe_list")]
    pub libraries: Vec<String>,
}

/// Queue and retry settings (`QueueSettings`).
///
/// The Python default for `db_path` is empty, with a root `model_validator`
/// later filling in `{XDG_DATA_HOME}/subgen/queue.db`. The captured golden
/// records the resolved-but-unexpanded form `${XDG_DATA_HOME}/subgen/queue.db`;
/// the actual expansion is part of the downstream resolution item.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct QueueSettings {
    pub db_path: String,
    pub max_retries: u32,
    pub retry_delay: u32,
}

impl Default for QueueSettings {
    fn default() -> Self {
        QueueSettings {
            db_path: "${XDG_DATA_HOME}/subgen/queue.db".to_string(),
            max_retries: 3,
            retry_delay: 5,
        }
    }
}

/// Subtitle generation and language settings (`SubtitleSettings`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SubtitleSettings {
    pub force_detected_language_to: String,
    pub append_credits: bool,
    pub skip_if_target_subtitle_exists: bool,
    pub skip_if_external_subtitles_exist: bool,
    pub skip_if_internal_subtitle_language: String,
    #[serde(deserialize_with = "deserialize_pipe_list")]
    pub skip_subtitle_languages: Vec<String>,
    #[serde(deserialize_with = "deserialize_pipe_list")]
    pub skip_if_audio_languages: Vec<String>,
    pub skip_unknown_language: bool,
    #[serde(deserialize_with = "deserialize_pipe_list")]
    pub preferred_audio_languages: Vec<String>,
    pub limit_to_preferred_audio_languages: bool,
    pub lrc_for_audio_files: bool,
    pub only_skip_if_subgen_subtitle: bool,
    pub skip_if_no_language_but_subtitles_exist: bool,
    pub language_naming_type: LanguageNamingType,
    pub include_subgen_marker: bool,
    pub include_model_in_filename: bool,
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
            language_naming_type: LanguageNamingType::Iso6392B,
            include_subgen_marker: false,
            include_model_in_filename: false,
        }
    }
}

/// Translation settings for LLM-backed subtitle translation (`TranslationSettings`).
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
        TranslationSettings {
            backend: TranslationBackend::Ollama,
            ollama_model: "llama3.2".to_string(),
            ollama_url: "http://localhost:11434".to_string(),
            anthropic_api_key: String::new(),
            claude_model: "claude-sonnet-4-20250514".to_string(),
            openai_api_key: String::new(),
            openai_model: "gpt-4o-mini".to_string(),
            gemini_api_key: String::new(),
            gemini_model: "gemini-2.0-flash".to_string(),
            chunk_size: 50,
        }
    }
}

/// Root application configuration (`Config`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub whisper: WhisperSettings,
    pub stable_ts: StableTsSettings,
    pub server: ServerSettings,
    pub path_mapping: PathMappingSettings,
    pub jellyfin: JellyfinSettings,
    pub queue: QueueSettings,
    pub subtitle: SubtitleSettings,
    pub translation: TranslationSettings,
    pub debug: bool,
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

/// Coerce a pipe-separated env string into a `Vec<String>`, or pass through an
/// already-typed sequence from the file/defaults layer.
///
/// Ports the `parse_pipe_separated_*` `mode="before"` validators in
/// `submate/config.py`: figment hands env vars to serde as bare strings, so
/// `"a|b|c"` must be split on `'|'`, each element `trim()`-med, and empty
/// elements dropped (`"a||b"` and a trailing `|` yield no blank entries). The
/// file/defaults layer instead supplies a real JSON array, which must still
/// deserialize unchanged — matching Python's permissive branch where a
/// non-string value is returned as-is.
fn deserialize_pipe_list<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    match Value::deserialize(deserializer)? {
        Value::String(s) => Ok(s
            .split('|')
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .map(str::to_string)
            .collect()),
        other => Vec::<String>::deserialize(other).map_err(serde::de::Error::custom),
    }
}

/// Coerce a JSON-string env value into a `Map`, or pass through an already-typed
/// map.
///
/// Ports `WhisperSettings.parse_json_kwargs`: `transcribe_kwargs` arrives from
/// the env as a JSON **string** (`'{"beam_size": 5}'`) which must be parsed into
/// a map; the file/defaults layer supplies a real object that passes through
/// unchanged. An absent field falls back to `#[serde(default)]` (`{}`); an empty
/// string also yields `{}`, matching the Python validator's empty-input branch.
fn deserialize_json_kwargs<'de, D>(deserializer: D) -> Result<Map<String, Value>, D::Error>
where
    D: Deserializer<'de>,
{
    match Value::deserialize(deserializer)? {
        Value::String(s) if s.is_empty() => Ok(Map::new()),
        Value::String(s) => serde_json::from_str(&s).map_err(serde::de::Error::custom),
        other => Map::<String, Value>::deserialize(other).map_err(serde::de::Error::custom),
    }
}

/// Coerce a regroup env value into a [`StrOrBool`], or pass through an
/// already-typed bool/string.
///
/// Ports `StableTsSettings.parse_regroup`: a string in `{false, off, 0, no, ""}`
/// (case-insensitive) disables regrouping (`Bool(false)`); any other string is a
/// regroup pattern (`Str(_)`). A real bool from the file/defaults layer passes
/// through unchanged.
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
