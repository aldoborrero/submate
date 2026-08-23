//! Shared enums.
//!
//! Variant string representations are fixed: the string forms are part of the
//! config/CLI/wire contract (env vars, JSON payloads, file suffixes), so they
//! must stay byte-for-byte stable. The non-identity cases are the dotted
//! Whisper `.en` models, the hyphenated `*-whisper` implementations, and the
//! `iso_639_*` language naming codes — see the literals below.
//!
//! `Display`/`FromStr` come from `strum` and serde `Serialize`/`Deserialize`
//! is derived from the same per-variant rename, so all four directions agree
//! on the exact string. Parity against the recorded values is enforced by
//! `tests/parity.rs` (falsifier `parity::enum_values`).

use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

/// Valid Whisper model sizes.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Display, EnumString, EnumIter, Serialize, Deserialize,
)]
pub enum WhisperModel {
    #[strum(serialize = "tiny")]
    #[serde(rename = "tiny")]
    Tiny,
    #[strum(serialize = "tiny.en")]
    #[serde(rename = "tiny.en")]
    TinyEn,
    #[strum(serialize = "base")]
    #[serde(rename = "base")]
    Base,
    #[strum(serialize = "base.en")]
    #[serde(rename = "base.en")]
    BaseEn,
    #[strum(serialize = "small")]
    #[serde(rename = "small")]
    Small,
    #[strum(serialize = "small.en")]
    #[serde(rename = "small.en")]
    SmallEn,
    #[strum(serialize = "medium")]
    #[serde(rename = "medium")]
    Medium,
    #[strum(serialize = "medium.en")]
    #[serde(rename = "medium.en")]
    MediumEn,
    #[strum(serialize = "large")]
    #[serde(rename = "large")]
    Large,
    #[strum(serialize = "large-v1")]
    #[serde(rename = "large-v1")]
    LargeV1,
    #[strum(serialize = "large-v2")]
    #[serde(rename = "large-v2")]
    LargeV2,
    #[strum(serialize = "large-v3")]
    #[serde(rename = "large-v3")]
    LargeV3,
}

/// Valid compute devices.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Display, EnumString, EnumIter, Serialize, Deserialize,
)]
pub enum Device {
    #[strum(serialize = "cpu")]
    #[serde(rename = "cpu")]
    Cpu,
    #[strum(serialize = "cuda")]
    #[serde(rename = "cuda")]
    Cuda,
    #[strum(serialize = "vulkan")]
    #[serde(rename = "vulkan")]
    Vulkan,
    #[strum(serialize = "auto")]
    #[serde(rename = "auto")]
    Auto,
}

/// Valid transcription tasks.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Display, EnumString, EnumIter, Serialize, Deserialize,
)]
pub enum TranscriptionTask {
    #[strum(serialize = "transcribe")]
    #[serde(rename = "transcribe")]
    Transcribe,
    #[strum(serialize = "translate")]
    #[serde(rename = "translate")]
    Translate,
}

/// Language code format for subtitle filenames.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Display, EnumString, EnumIter, Serialize, Deserialize,
)]
pub enum LanguageNamingType {
    /// 2-letter: `en`, `es`, `de`.
    #[strum(serialize = "iso_639_1")]
    #[serde(rename = "iso_639_1")]
    Iso6391,
    /// 3-letter terminological: `eng`, `spa`, `deu`.
    #[strum(serialize = "iso_639_2_t")]
    #[serde(rename = "iso_639_2_t")]
    Iso6392T,
    /// 3-letter bibliographic: `eng`, `spa`, `ger`.
    #[strum(serialize = "iso_639_2_b")]
    #[serde(rename = "iso_639_2_b")]
    Iso6392B,
    /// English name: `English`, `Spanish`, `German`.
    #[strum(serialize = "name")]
    #[serde(rename = "name")]
    Name,
    /// Native name: `English`, `Español`, `Deutsch`.
    #[strum(serialize = "native")]
    #[serde(rename = "native")]
    Native,
}

/// LLM backends for subtitle translation.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Display, EnumString, EnumIter, Serialize, Deserialize,
)]
pub enum TranslationBackend {
    /// Local, free, private.
    #[strum(serialize = "ollama")]
    #[serde(rename = "ollama")]
    Ollama,
    /// Anthropic Claude API.
    #[strum(serialize = "claude")]
    #[serde(rename = "claude")]
    Claude,
    /// OpenAI API.
    #[strum(serialize = "openai")]
    #[serde(rename = "openai")]
    Openai,
    /// Google Gemini API.
    #[strum(serialize = "gemini")]
    #[serde(rename = "gemini")]
    Gemini,
}

/// Subtitle output format a transcription job emits.
///
/// The wire/value string is the bare lowercase format name (`"srt"`, `"vtt"`,
/// …) and [`OutputFormat::extension`] prepends the dot. Defaults to
/// [`OutputFormat::Srt`] so a payload without an explicit format produces SRT.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Default,
    Display,
    EnumString,
    EnumIter,
    Serialize,
    Deserialize,
)]
pub enum OutputFormat {
    /// SubRip subtitles (`srt`).
    #[default]
    #[strum(serialize = "srt")]
    #[serde(rename = "srt")]
    Srt,
    /// WebVTT subtitles (`vtt`).
    #[strum(serialize = "vtt")]
    #[serde(rename = "vtt")]
    Vtt,
    /// Advanced SubStation Alpha subtitles (`ass`).
    #[strum(serialize = "ass")]
    #[serde(rename = "ass")]
    Ass,
    /// JSON dump of the full transcription result (`json`).
    #[strum(serialize = "json")]
    #[serde(rename = "json")]
    Json,
    /// Plain-text transcript, no timestamps (`txt`).
    #[strum(serialize = "txt")]
    #[serde(rename = "txt")]
    Txt,
}

impl OutputFormat {
    /// The on-the-wire value string (e.g. `"srt"`).
    pub fn value(self) -> &'static str {
        match self {
            Self::Srt => "srt",
            Self::Vtt => "vtt",
            Self::Ass => "ass",
            Self::Json => "json",
            Self::Txt => "txt",
        }
    }

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

    /// Coerce an optional string into an `OutputFormat`, never erroring: a known
    /// format name maps to its variant; anything else (or `None`) falls back to
    /// `default` when given, otherwise [`OutputFormat::Srt`].
    pub fn from_value(value: Option<&str>, default: Option<Self>) -> Self {
        match value {
            Some("srt") => Self::Srt,
            Some("vtt") => Self::Vtt,
            Some("ass") => Self::Ass,
            Some("json") => Self::Json,
            Some("txt") => Self::Txt,
            _ => default.unwrap_or(Self::Srt),
        }
    }
}

#[cfg(test)]
mod output_format_tests {
    use super::OutputFormat;

    #[test]
    fn value_and_extension() {
        assert_eq!(OutputFormat::Srt.value(), "srt");
        assert_eq!(OutputFormat::Ass.value(), "ass");
        assert_eq!(OutputFormat::Srt.extension(), ".srt");
        assert_eq!(OutputFormat::Ass.extension(), ".ass");
        assert_eq!(OutputFormat::default(), OutputFormat::Srt);
    }

    #[test]
    fn from_value_never_errors() {
        assert_eq!(
            OutputFormat::from_value(Some("vtt"), None),
            OutputFormat::Vtt
        );
        assert_eq!(
            OutputFormat::from_value(Some("ass"), None),
            OutputFormat::Ass
        );
        // Unknown / None fall back to the given default, else Srt.
        assert_eq!(
            OutputFormat::from_value(Some("nope"), None),
            OutputFormat::Srt
        );
        assert_eq!(
            OutputFormat::from_value(Some("nope"), Some(OutputFormat::Txt)),
            OutputFormat::Txt
        );
        assert_eq!(OutputFormat::from_value(None, None), OutputFormat::Srt);
    }
}
