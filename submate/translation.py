"""Translation service for multi-language subtitle translation via LLM APIs."""

import logging
import re
from abc import ABC, abstractmethod

import srt

from submate.config import Config
from submate.types import TranslationBackend

logger = logging.getLogger(__name__)

# Translation prompt template
TRANSLATION_PROMPT = """Translate the following subtitle text from {source_lang} to {target_lang}.

Rules:
- Only output the translated text, nothing else
- Preserve line breaks where they appear
- Maintain natural speech patterns suitable for subtitles
- Keep the same number of subtitle blocks (separated by ---BREAK---)

Text to translate:
{text}"""

# ASS translation prompt - preserves formatting tags
ASS_TRANSLATION_PROMPT = """Translate the following ASS subtitle dialogue from {source_lang} to {target_lang}.

CRITICAL RULES:
1. ONLY translate the human-readable dialogue text
2. PRESERVE ALL formatting tags exactly as-is: {{\\i1}}, {{\\b1}}, {{\\pos(x,y)}}, {{\\an8}}, {{\\fad(x,y)}}, etc.
3. PRESERVE newline markers: \\N and \\n
4. PRESERVE the exact line structure (one subtitle per line, separated by |||SUBTITLE_BREAK|||)
5. DO NOT add, remove, or modify any tags inside curly braces {{}}
6. DO NOT translate or modify anything inside curly braces {{}}
7. Output ONLY the translated subtitles, no explanations

Example input:
{{\\i1}}Bonjour{{\\i0}} monde
|||SUBTITLE_BREAK|||
{{\\an8}}Comment ça va?

Example output:
{{\\i1}}Hello{{\\i0}} world
|||SUBTITLE_BREAK|||
{{\\an8}}How are you?

Subtitles to translate:
{text}"""


def validate_ass_tags(original: str, translated: str) -> bool:
    """Validate that ASS formatting tags are preserved after translation.

    Args:
        original: Original text with ASS tags
        translated: Translated text that should have same tags

    Returns:
        True if tags match exactly, False otherwise
    """
    tag_pattern = re.compile(r"\{[^}]*\}")
    original_tags = tag_pattern.findall(original)
    translated_tags = tag_pattern.findall(translated)
    return original_tags == translated_tags


class TranslationBackendBase(ABC):
    """Abstract base class for translation backends."""

    @abstractmethod
    def translate(self, text: str, source_lang: str, target_lang: str, prompt_template: str | None = None) -> str:
        """Translate text from source language to target language."""
        pass


class OllamaBackend(TranslationBackendBase):
    """Ollama-based translation backend (local, free, private)."""

    def __init__(self, model: str = "llama3.2", base_url: str = "http://localhost:11434"):
        self.model = model
        self.base_url = base_url

    def translate(self, text: str, source_lang: str, target_lang: str, prompt_template: str | None = None) -> str:
        try:
            import ollama
        except ImportError as e:
            raise ImportError("ollama package not installed. Install with: pip install submate[ollama]") from e

        client = ollama.Client(host=self.base_url)
        template = prompt_template or TRANSLATION_PROMPT
        prompt = template.format(source_lang=source_lang, target_lang=target_lang, text=text)

        response = client.chat(
            model=self.model,
            messages=[{"role": "user", "content": prompt}],
        )

        return str(response["message"]["content"]).strip()


class ClaudeBackend(TranslationBackendBase):
    """Claude/Anthropic-based translation backend."""

    def __init__(self, api_key: str, model: str = "claude-sonnet-4-20250514"):
        self.api_key = api_key
        self.model = model

    def translate(self, text: str, source_lang: str, target_lang: str, prompt_template: str | None = None) -> str:
        try:
            import anthropic
        except ImportError as e:
            raise ImportError("anthropic package not installed. Install with: pip install submate[claude]") from e

        client = anthropic.Anthropic(api_key=self.api_key)
        template = prompt_template or TRANSLATION_PROMPT
        prompt = template.format(source_lang=source_lang, target_lang=target_lang, text=text)

        message = client.messages.create(
            model=self.model,
            max_tokens=4096,
            messages=[{"role": "user", "content": prompt}],
        )

        # Extract text from TextBlock (filter out other block types)
        for block in message.content:
            if hasattr(block, "text"):
                return str(block.text).strip()
        return ""


class OpenAIBackend(TranslationBackendBase):
    """OpenAI-based translation backend."""

    def __init__(self, api_key: str, model: str = "gpt-4o-mini"):
        self.api_key = api_key
        self.model = model

    def translate(self, text: str, source_lang: str, target_lang: str, prompt_template: str | None = None) -> str:
        try:
            import openai
        except ImportError as e:
            raise ImportError("openai package not installed. Install with: pip install submate[openai]") from e

        client = openai.OpenAI(api_key=self.api_key)
        template = prompt_template or TRANSLATION_PROMPT
        prompt = template.format(source_lang=source_lang, target_lang=target_lang, text=text)

        response = client.chat.completions.create(
            model=self.model,
            messages=[{"role": "user", "content": prompt}],
        )

        content = response.choices[0].message.content or ""
        return content.strip()


class GeminiBackend(TranslationBackendBase):
    """Google Gemini-based translation backend."""

    def __init__(self, api_key: str, model: str = "gemini-2.0-flash"):
        self.api_key = api_key
        self.model = model

    def translate(self, text: str, source_lang: str, target_lang: str, prompt_template: str | None = None) -> str:
        try:
            from google import genai  # type: ignore[attr-defined]
        except ImportError as e:
            raise ImportError("google-genai package not installed. Install with: pip install submate[gemini]") from e

        client = genai.Client(api_key=self.api_key)
        template = prompt_template or TRANSLATION_PROMPT
        prompt = template.format(source_lang=source_lang, target_lang=target_lang, text=text)

        response = client.models.generate_content(model=self.model, contents=prompt)

        return str(response.text).strip()


class TranslationService:
    """Service for translating subtitle text using configurable LLM backends."""

    def __init__(self, config: Config):
        self.config = config
        self.backend = self._init_backend()

    def _init_backend(self) -> "TranslationBackendBase":
        """Initialize the appropriate translation backend based on config."""
        settings = self.config.translation

        match settings.backend:
            case TranslationBackend.OLLAMA:
                logger.debug("Using Ollama backend with model: %s", settings.ollama_model)
                return OllamaBackend(settings.ollama_model, settings.ollama_url)
            case TranslationBackend.CLAUDE:
                if not settings.anthropic_api_key:
                    raise ValueError("TRANSLATION__ANTHROPIC_API_KEY required for Claude backend")
                logger.debug("Using Claude backend with model: %s", settings.claude_model)
                return ClaudeBackend(settings.anthropic_api_key, settings.claude_model)
            case TranslationBackend.OPENAI:
                if not settings.openai_api_key:
                    raise ValueError("TRANSLATION__OPENAI_API_KEY required for OpenAI backend")
                logger.debug("Using OpenAI backend with model: %s", settings.openai_model)
                return OpenAIBackend(settings.openai_api_key, settings.openai_model)
            case TranslationBackend.GEMINI:
                if not settings.gemini_api_key:
                    raise ValueError("TRANSLATION__GEMINI_API_KEY required for Gemini backend")
                logger.debug("Using Gemini backend with model: %s", settings.gemini_model)
                return GeminiBackend(settings.gemini_api_key, settings.gemini_model)
            case _:
                raise ValueError(f"Unknown translation backend: {settings.backend}")

    def translate_text(self, text: str, source_lang: str, target_lang: str, prompt_template: str | None = None) -> str:
        """Translate plain text.

        Args:
            text: Text to translate
            source_lang: Source language code
            target_lang: Target language code
            prompt_template: Optional custom prompt template (must contain {source_lang}, {target_lang}, {text})

        Returns:
            Translated text
        """
        if source_lang == target_lang:
            logger.debug("Source and target languages are the same, skipping translation")
            return text

        logger.info("Translating from %s to %s", source_lang, target_lang)
        return self.backend.translate(text, source_lang, target_lang, prompt_template=prompt_template)

    def translate_subtitles(
        self, subtitles: list[srt.Subtitle], source_lang: str, target_lang: str
    ) -> list[srt.Subtitle]:
        """Translate a list of SRT subtitles.

        Uses chunked batch translation to stay within context window limits
        while maintaining context within each chunk.

        Args:
            subtitles: List of srt.Subtitle objects
            source_lang: Source language code
            target_lang: Target language code

        Returns:
            List of translated srt.Subtitle objects
        """
        if source_lang == target_lang:
            return subtitles

        if not subtitles:
            return subtitles

        chunk_size = self.config.translation.chunk_size
        total_chunks = (len(subtitles) + chunk_size - 1) // chunk_size

        logger.info(
            "Translating %d subtitle blocks from %s to %s in %d chunk(s) of %d",
            len(subtitles),
            source_lang,
            target_lang,
            total_chunks,
            chunk_size,
        )

        translated_subtitles = []

        for chunk_idx in range(total_chunks):
            start_idx = chunk_idx * chunk_size
            end_idx = min(start_idx + chunk_size, len(subtitles))
            chunk = subtitles[start_idx:end_idx]

            logger.debug(
                "Processing chunk %d/%d (subtitles %d-%d)", chunk_idx + 1, total_chunks, start_idx + 1, end_idx
            )

            # Translate this chunk
            translated_chunk = self._translate_chunk(chunk, source_lang, target_lang)
            translated_subtitles.extend(translated_chunk)

        return translated_subtitles

    def _translate_chunk(self, chunk: list[srt.Subtitle], source_lang: str, target_lang: str) -> list[srt.Subtitle]:
        """Translate a single chunk of subtitles.

        Args:
            chunk: List of srt.Subtitle objects to translate
            source_lang: Source language code
            target_lang: Target language code

        Returns:
            List of translated srt.Subtitle objects
        """
        separator = "\n---BREAK---\n"
        texts = [sub.content for sub in chunk]
        combined_text = separator.join(texts)

        # Translate the chunk
        translated_combined = self.backend.translate(combined_text, source_lang, target_lang)

        # Split back into individual blocks
        translated_texts = translated_combined.split("---BREAK---")
        translated_texts = [t.strip() for t in translated_texts]

        # Create new subtitle objects with translated content
        translated_chunk = []
        for i, sub in enumerate(chunk):
            translated_content = translated_texts[i] if i < len(translated_texts) else sub.content
            translated_chunk.append(
                srt.Subtitle(
                    index=sub.index,
                    start=sub.start,
                    end=sub.end,
                    content=translated_content,
                    proprietary=sub.proprietary,
                )
            )

        return translated_chunk

    def translate_srt_content(self, srt_content: str, source_lang: str, target_lang: str) -> str:
        """Translate SRT content string.

        Parses SRT, translates subtitles, and returns composed SRT string.

        Args:
            srt_content: Raw SRT file content
            source_lang: Source language code
            target_lang: Target language code

        Returns:
            Translated SRT content string
        """
        if source_lang == target_lang:
            return srt_content

        # Parse SRT content
        subtitles = list(srt.parse(srt_content))

        # Translate
        translated_subtitles = self.translate_subtitles(subtitles, source_lang, target_lang)

        # Compose back to SRT string
        return str(srt.compose(translated_subtitles))

    def translate_ass_content(self, ass_content: str, source_lang: str, target_lang: str) -> str:
        """Translate ASS/SSA subtitle content while preserving formatting tags.

        Uses pysubs2 to parse ASS, extracts dialogue text with tags intact,
        translates via LLM with tag-preservation prompt, and reconstructs ASS.

        Args:
            ass_content: Raw ASS/SSA file content
            source_lang: Source language code
            target_lang: Target language code

        Returns:
            Translated ASS content string with preserved formatting
        """
        import pysubs2

        if source_lang == target_lang:
            return ass_content

        # Parse ASS content
        subs = pysubs2.SSAFile.from_string(ass_content)

        # Extract dialogue events with text
        events_to_translate = [(i, event) for i, event in enumerate(subs) if event.is_text and event.text.strip()]

        if not events_to_translate:
            return ass_content

        # Translate in chunks
        chunk_size = self.config.translation.chunk_size
        separator = "\n|||SUBTITLE_BREAK|||\n"

        for chunk_start in range(0, len(events_to_translate), chunk_size):
            chunk = events_to_translate[chunk_start : chunk_start + chunk_size]

            # Combine texts for batch translation
            texts = [event.text for _, event in chunk]
            combined = separator.join(texts)

            # Translate with ASS-specific prompt
            translated = self.backend.translate(
                combined, source_lang, target_lang, prompt_template=ASS_TRANSLATION_PROMPT
            )

            # Split back and apply
            translated_texts = translated.split("|||SUBTITLE_BREAK|||")
            translated_texts = [t.strip() for t in translated_texts]

            for j, (idx, event) in enumerate(chunk):
                if j < len(translated_texts):
                    new_text = translated_texts[j]
                    # Validate tags are preserved
                    if validate_ass_tags(event.text, new_text):
                        subs[idx].text = new_text
                    else:
                        logger.warning(
                            "Tag mismatch in subtitle %d, keeping original. Original: %r, Translated: %r",
                            idx,
                            event.text,
                            new_text,
                        )

        return subs.to_string("ass")
