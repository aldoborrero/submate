//! Pure-data result-routing enums.
//!
//! These two enums are the vocabulary the whole server↔node system speaks
//! when reporting transcription outcomes, so their wire strings are frozen.
//! Each variant carries an explicit `#[serde(rename = "...")]` so a naive
//! derive can never mangle a snake_case `not_skipped` into `NotSkipped`.
//!
//! Parity against the captured values is enforced by `tests/parity.rs`
//! (falsifier `parity::queue_enum_values`).

use serde::{Deserialize, Serialize};
use strum::EnumIter;

/// Subtitle output format for a job. The canonical enum lives in `submate-types`
/// (one definition shared across the wire, queue and CLI layers); it is
/// re-exported here so `submate_queue::OutputFormat` keeps resolving.
pub use submate_types::OutputFormat;

/// Reasons a transcription was skipped.
///
/// The `.value` strings are the literal `reason` field returned in the worker
/// task envelope. Every variant's wire string is pinned with
/// `#[serde(rename = "...")]`.
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
    /// The on-the-wire `.value` string.
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
