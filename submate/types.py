"""Type definitions and enums."""

from enum import StrEnum


class WhisperModel(StrEnum):
    """Valid Whisper model sizes."""

    TINY = "tiny"
    TINY_EN = "tiny.en"
    BASE = "base"
    BASE_EN = "base.en"
    SMALL = "small"
    SMALL_EN = "small.en"
    MEDIUM = "medium"
    MEDIUM_EN = "medium.en"
    LARGE = "large"
    LARGE_V1 = "large-v1"
    LARGE_V2 = "large-v2"
    LARGE_V3 = "large-v3"


class WhisperImplementation(StrEnum):
    """Valid Whisper implementations."""

    FASTER_WHISPER = "faster-whisper"
    OPENAI_WHISPER = "openai-whisper"
    HF_WHISPER = "hf-whisper"


class Device(StrEnum):
    """Valid compute devices."""

    CPU = "cpu"
    CUDA = "cuda"
    AUTO = "auto"


class TranscriptionTask(StrEnum):
    """Valid transcription tasks."""

    TRANSCRIBE = "transcribe"
    TRANSLATE = "translate"


class LanguageNamingType(StrEnum):
    """Language code format for subtitle filenames."""

    ISO_639_1 = "iso_639_1"  # 2-letter: en, es, de
    ISO_639_2_T = "iso_639_2_t"  # 3-letter terminological: eng, spa, deu
    ISO_639_2_B = "iso_639_2_b"  # 3-letter bibliographic: eng, spa, ger
    NAME = "name"  # English name: English, Spanish, German
    NATIVE = "native"  # Native name: English, Español, Deutsch


class TranslationBackend(StrEnum):
    """LLM backends for subtitle translation."""

    OLLAMA = "ollama"  # Local, free, private
    CLAUDE = "claude"  # Anthropic Claude API
    OPENAI = "openai"  # OpenAI API
    GEMINI = "gemini"  # Google Gemini API
