//! Shared enums.
//!
//! Each enum mirrors a Python `StrEnum`. Variant string representations must
//! match Python's `.value` byte-for-byte, because these strings cross the
//! config/CLI/wire boundary (Pydantic-Settings env vars, JSON payloads, file
//! suffixes). The non-identity cases are the dotted Whisper `.en` models, the
//! hyphenated `*-whisper` implementations, and the `iso_639_*` language naming
//! codes — see the literals below.
//!
//! `Display`/`FromStr` come from `strum` and serde `Serialize`/`Deserialize`
//! is derived from the same per-variant rename, so all four directions agree
//! on the exact Python string. Parity against the captured Python values is
//! enforced by `tests/parity.rs` (falsifier `parity::enum_values`).

use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

/// Valid Whisper model sizes (`submate.types.WhisperModel`).
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

/// Valid Whisper implementations (`submate.types.WhisperImplementation`).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Display, EnumString, EnumIter, Serialize, Deserialize,
)]
pub enum WhisperImplementation {
    #[strum(serialize = "faster-whisper")]
    #[serde(rename = "faster-whisper")]
    FasterWhisper,
    #[strum(serialize = "openai-whisper")]
    #[serde(rename = "openai-whisper")]
    OpenaiWhisper,
    #[strum(serialize = "hf-whisper")]
    #[serde(rename = "hf-whisper")]
    HfWhisper,
}

/// Valid compute devices (`submate.types.Device`).
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
    #[strum(serialize = "auto")]
    #[serde(rename = "auto")]
    Auto,
}

/// Valid transcription tasks (`submate.types.TranscriptionTask`).
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

/// Language code format for subtitle filenames
/// (`submate.types.LanguageNamingType`).
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

/// LLM backends for subtitle translation (`submate.types.TranslationBackend`).
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
