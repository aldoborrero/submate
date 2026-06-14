//! Pure-data result-routing enums (ports `submate/queue/models.py`).
//!
//! These two enums are the vocabulary the whole server↔node system speaks
//! when reporting transcription outcomes, so their wire strings must match
//! Python's `.value` byte-for-byte. Each variant carries an explicit
//! `#[serde(rename = "...")]` so a naive derive can never mangle a snake_case
//! `not_skipped` into `NotSkipped`.
//!
//! Parity against the captured Python values is enforced by
//! `tests/parity.rs` (falsifier `parity::queue_enum_values`).

use serde::{Deserialize, Serialize};
use strum::EnumIter;

/// Supported output formats for transcription (`OutputFormat` in
/// `submate.queue.models`).
///
/// A plain `Enum` on the Python side — its `.value` is the lowercase format
/// name, and `.extension` prepends a dot. Coercion from arbitrary strings goes
/// through [`OutputFormat::from_value`], which never errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, EnumIter, Serialize, Deserialize)]
pub enum OutputFormat {
    /// SubRip subtitles (`srt`).
    #[serde(rename = "srt")]
    Srt,
    /// WebVTT subtitles (`vtt`).
    #[serde(rename = "vtt")]
    Vtt,
    /// Plain text (`txt`).
    #[serde(rename = "txt")]
    Txt,
    /// JSON segments (`json`).
    #[serde(rename = "json")]
    Json,
}

impl OutputFormat {
    /// The on-the-wire `.value` string (matches Python `OutputFormat.value`).
    pub fn value(self) -> &'static str {
        match self {
            Self::Srt => "srt",
            Self::Vtt => "vtt",
            Self::Txt => "txt",
            Self::Json => "json",
        }
    }

    /// File extension including the leading dot (e.g. `".srt"`).
    ///
    /// Mirrors Python's `OutputFormat.extension` (`f".{value}"`).
    pub fn extension(self) -> String {
        format!(".{}", self.value())
    }

    /// Coerce an optional string into an `OutputFormat`, never erroring.
    ///
    /// Ports `OutputFormat.from_value`: a known string maps to its variant; an
    /// unknown (or `None`) string falls back to `default` if given, else
    /// [`OutputFormat::Srt`]. (The Python overload that accepts an existing
    /// `OutputFormat` is the identity and needs no Rust counterpart.)
    pub fn from_value(value: Option<&str>, default: Option<Self>) -> Self {
        match value {
            Some("srt") => Self::Srt,
            Some("vtt") => Self::Vtt,
            Some("txt") => Self::Txt,
            Some("json") => Self::Json,
            _ => default.unwrap_or(Self::Srt),
        }
    }
}

/// Reasons a transcription was skipped (`SkipReason` in
/// `submate.queue.models`).
///
/// A Python `StrEnum`, so the `.value` strings are the literal `reason` field
/// returned in the worker task envelope. Every variant's wire string is pinned
/// with `#[serde(rename = "...")]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, EnumIter, Serialize, Deserialize)]
pub enum SkipReason {
    /// Not skipped.
    #[serde(rename = "not_skipped")]
    NotSkipped,
    /// The target subtitle file already exists.
    #[serde(rename = "target_subtitle_exists")]
    TargetSubtitleExists,
    /// An external subtitle file already exists.
    #[serde(rename = "external_subtitle_exists")]
    ExternalSubtitleExists,
    /// An internal subtitle track for the language already exists.
    #[serde(rename = "internal_subtitle_language_exists")]
    InternalSubtitleLanguageExists,
    /// The subtitle language is in the skip list.
    #[serde(rename = "subtitle_language_in_skip_list")]
    SubtitleLanguageInSkipList,
    /// The audio language is in the skip list.
    #[serde(rename = "audio_language_in_skip_list")]
    AudioLanguageInSkipList,
    /// The language could not be determined.
    #[serde(rename = "unknown_language")]
    UnknownLanguage,
    /// No preferred audio language matched.
    #[serde(rename = "no_preferred_audio_language")]
    NoPreferredAudioLanguage,
    /// An `.lrc` lyrics file already exists.
    #[serde(rename = "lrc_file_exists")]
    LrcFileExists,
    /// The language is unset but subtitles already exist.
    #[serde(rename = "language_not_set_but_subtitles_exist")]
    LanguageNotSetButSubtitlesExist,
}

impl SkipReason {
    /// The on-the-wire `.value` string (matches Python `SkipReason.value`).
    pub fn value(self) -> &'static str {
        match self {
            Self::NotSkipped => "not_skipped",
            Self::TargetSubtitleExists => "target_subtitle_exists",
            Self::ExternalSubtitleExists => "external_subtitle_exists",
            Self::InternalSubtitleLanguageExists => "internal_subtitle_language_exists",
            Self::SubtitleLanguageInSkipList => "subtitle_language_in_skip_list",
            Self::AudioLanguageInSkipList => "audio_language_in_skip_list",
            Self::UnknownLanguage => "unknown_language",
            Self::NoPreferredAudioLanguage => "no_preferred_audio_language",
            Self::LrcFileExists => "lrc_file_exists",
            Self::LanguageNotSetButSubtitlesExist => "language_not_set_but_subtitles_exist",
        }
    }
}
