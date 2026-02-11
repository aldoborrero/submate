"""stable-whisper integration for enhanced subtitle generation.

This module provides a thread-safe, resource-managed wrapper around the
stable-whisper library with proper lifecycle management via context manager.
"""

import gc
import logging
import os
import tempfile
import threading
import wave
from io import BytesIO
from pathlib import Path
from typing import Any, Literal, Protocol, cast

import stable_whisper

from submate.config import Config
from submate.types import TranscriptionTask, WhisperModel

logger = logging.getLogger(__name__)


# Type Protocols
class TranscriptionResult(Protocol):
    """Protocol for stable-whisper transcription result.

    This defines the interface we expect from the result object.
    """

    language: str
    text: str
    segments: list[Any]

    def to_srt_vtt(
        self,
        filepath: str | None = None,
        word_level: bool = False,
        vtt: bool = False,
    ) -> str | None:
        """Export to SRT/VTT format."""
        ...

    def to_dict(self) -> dict[str, Any]:
        """Convert to dictionary."""
        ...


class StableWhisperModel(Protocol):
    """Protocol for stable-whisper model interface."""

    def transcribe_stable(
        self,
        audio: str | bytes,
        regroup: bool | str = True,
        language: str | None = None,
        task: str = "transcribe",
        **kwargs: Any,
    ) -> TranscriptionResult:
        """Transcribe with stable-whisper enhancements."""
        ...


class WhisperModelWrapper:
    """Thread-safe Whisper model wrapper with automatic lifecycle management.

    Designed for use in Huey tasks with context manager protocol:
        with WhisperModelWrapper(config) as model:
            result = model.transcribe(audio)

    Thread Safety:
        All public methods are thread-safe via internal RLock.

    Resource Management:
        Use context manager for automatic cleanup, or call load()/unload() manually.
    """

    # Valid model sizes (now sourced from enum)
    VALID_MODELS = {model.value for model in WhisperModel}
    VALID_TASKS = {task.value for task in TranscriptionTask}

    def __init__(self, config: Config) -> None:
        """Initialize model wrapper with config.

        Args:
            config: Application configuration (pre-validated)
        """
        # Config is already validated by Pydantic - no need for additional validation
        self._config = config
        self._model: Any = None
        self._lock = threading.RLock()
        self._loaded = False
        self._temp_audio_file: str | None = None

    @property
    def is_loaded(self) -> bool:
        """Check if model is currently loaded.

        Thread-safe property.
        """
        with self._lock:
            return self._loaded

    @property
    def config(self) -> Config:
        """Get the configuration this model was created with."""
        return self._config

    def __repr__(self) -> str:
        """Developer-friendly representation."""
        status = "loaded" if self._loaded else "unloaded"
        return (
            f"WhisperModelWrapper("
            f"model={self._config.whisper.model!r}, "
            f"device={self._config.whisper.device!r}, "
            f"status={status})"
        )

    def __str__(self) -> str:
        """User-friendly representation."""
        status = "loaded" if self._loaded else "unloaded"
        return f"WhisperModelWrapper[{self._config.whisper.model}]: {status}"

    def load(self) -> None:
        """Load the Whisper model into memory.

        Idempotent - calling multiple times is safe.
        Thread-safe.

        Raises:
            RuntimeError: If model loading fails
        """
        with self._lock:
            if self._loaded:
                logger.debug(f"Model already loaded: {self._config.whisper.model}")
                return

            logger.info(f"Loading stable-whisper model: {self._config.whisper.model}")

            try:
                self._model = stable_whisper.load_faster_whisper(
                    self._config.whisper.model,
                    device=self._config.whisper.device,
                    compute_type=self._config.whisper.compute_type,
                )

                self._loaded = True
                logger.info("Model loaded successfully")

            except Exception as e:
                logger.error(f"Failed to load model: {e}", exc_info=True)
                raise RuntimeError(f"Failed to load model: {e}") from e

    def unload(self) -> None:
        """Unload model and free all resources.

        Idempotent - calling multiple times is safe.
        Thread-safe.
        """
        with self._lock:
            if not self._loaded or self._model is None:
                logger.debug("Model not loaded, nothing to unload")
                return

            logger.info("Unloading model and freeing resources")

            # Release model reference
            self._model = None
            self._loaded = False

            # Force garbage collection
            gc.collect()
            logger.debug("Garbage collection completed")

            # Clear VRAM if configured
            if self._config.clear_vram_on_complete:
                self._clear_vram()

    def _clear_vram(self) -> None:
        """Clear CUDA VRAM.

        Called during unload if config.clear_vram_on_complete=True.
        """
        try:
            import torch

            if torch.cuda.is_available():
                torch.cuda.empty_cache()
                torch.cuda.ipc_collect()
                logger.info("VRAM cleared")
            else:
                logger.debug("CUDA not available, skipping VRAM clear")

        except ImportError:
            logger.warning("torch not available, cannot clear VRAM")
        except Exception as e:
            logger.error(f"Failed to clear VRAM: {e}", exc_info=True)

    def transcribe(
        self,
        audio: Path | str | bytes | BytesIO,
        *,
        language: str | None = None,
        task: Literal["transcribe", "translate"] = "transcribe",
        **extra_kwargs: Any,
    ) -> TranscriptionResult:
        """Transcribe audio using stable-whisper.

        Thread-safe.

        Args:
            audio: Audio input (file path, bytes, or BytesIO)
            language: Language code (e.g., "en", "es"). Auto-detect if None
            task: "transcribe" (same language) or "translate" (to English)
            **extra_kwargs: Additional kwargs passed to transcribe_stable()

        Returns:
            TranscriptionResult with .text, .language, .segments, .to_srt_vtt()

        Raises:
            RuntimeError: If model not loaded or transcription fails
            ValueError: If task is invalid
        """
        # Validate task
        if task not in self.VALID_TASKS:
            raise ValueError(f"Invalid task: {task}. Valid options: {', '.join(self.VALID_TASKS)}")

        with self._lock:
            # Ensure model is loaded
            if not self._loaded or self._model is None:
                raise RuntimeError("Model not loaded. Use context manager or call .load() first")

            # Prepare audio input
            audio_input = self._prepare_audio(audio)

            # Log safely (no full paths)
            audio_desc = self._audio_description(audio)
            logger.info(f"Transcribing {audio_desc} (task={task}, language={language or 'auto'})")

            # Build options - start with config-defined kwargs, then override with explicit args
            options: dict[str, Any] = {**self._config.whisper.transcribe_kwargs}
            options["task"] = task
            if language:
                options["language"] = language
            # Extra kwargs override everything
            options.update(extra_kwargs)

            if options:
                logger.debug(f"Transcribe options: {options}")

            # Transcribe
            try:
                result = self._model.transcribe_stable(
                    audio_input,
                    regroup=self._config.stable_ts.custom_regroup,
                    **options,
                )

                if result is None:
                    raise RuntimeError("Transcription returned None")

                logger.info(f"Transcription complete (detected_language={getattr(result, 'language', 'unknown')})")
                return cast(TranscriptionResult, result)

            except Exception as e:
                logger.error(f"Transcription failed: {e}", exc_info=True)
                raise RuntimeError(f"Transcription failed: {e}") from e

            finally:
                # Clean up temp audio file if created
                self._cleanup_temp_audio()

    def _cleanup_temp_audio(self) -> None:
        """Clean up temporary audio file if one was created."""
        if self._temp_audio_file and os.path.exists(self._temp_audio_file):
            try:
                os.unlink(self._temp_audio_file)
                logger.debug(f"Cleaned up temp audio file: {self._temp_audio_file}")
            except OSError as e:
                logger.warning(f"Failed to clean up temp audio file: {e}")
            finally:
                self._temp_audio_file = None

    def _prepare_audio(self, audio: Path | str | bytes | BytesIO) -> str:
        """Convert audio input to format stable-whisper expects.

        stable-whisper only accepts file paths (strings), not BytesIO or raw bytes.
        For bytes/BytesIO input, we save to a temp file and return the path.

        Bazarr sends raw PCM audio (s16le, mono, 16kHz) without WAV headers,
        so we add proper WAV headers before saving.

        Args:
            audio: Raw audio input

        Returns:
            File path string (stable-whisper requirement)
        """
        if isinstance(audio, bytes):
            return self._save_audio_with_wav_headers(audio)
        elif isinstance(audio, BytesIO):
            audio.seek(0)
            return self._save_audio_with_wav_headers(audio.read())
        else:
            return str(audio)

    def _save_audio_with_wav_headers(self, pcm_data: bytes) -> str:
        """Save raw PCM data as a proper WAV file.

        Bazarr sends PCM audio in s16le format (signed 16-bit little-endian),
        mono channel, 16kHz sample rate. We add WAV headers so ffmpeg/PyAV
        can decode it.

        Args:
            pcm_data: Raw PCM audio bytes

        Returns:
            Path to temporary WAV file
        """
        temp_file = tempfile.NamedTemporaryFile(suffix=".wav", delete=False)
        temp_path = temp_file.name
        temp_file.close()

        # Check if data already has WAV header (starts with "RIFF")
        if pcm_data[:4] == b"RIFF":
            # Already a WAV file, just save it
            with open(temp_path, "wb") as f:
                f.write(pcm_data)
        else:
            # Raw PCM - add WAV headers
            # Bazarr format: s16le (signed 16-bit LE), mono, 16kHz
            channels = 1
            sample_rate = 16000
            sample_width = 2  # 16-bit = 2 bytes

            with wave.open(temp_path, "wb") as wav_file:
                wav_file.setnchannels(channels)
                wav_file.setsampwidth(sample_width)
                wav_file.setframerate(sample_rate)
                wav_file.writeframes(pcm_data)

        self._temp_audio_file = temp_path
        return temp_path

    def _audio_description(self, audio: Path | str | bytes | BytesIO) -> str:
        """Get safe description of audio for logging.

        Avoids logging full file paths (security concern).

        Args:
            audio: Audio input

        Returns:
            Safe description string
        """
        if isinstance(audio, bytes):
            return f"<bytes: {len(audio):,} bytes>"
        elif isinstance(audio, BytesIO):
            return f"<BytesIO: {len(audio.getvalue()):,} bytes>"
        else:
            # Only log filename, not full path
            return f"<file: {Path(audio).name}>"

    def __enter__(self) -> "WhisperModelWrapper":
        """Enter context manager - load model.

        Returns:
            Self for use in with statement
        """
        self.load()
        return self

    def __exit__(
        self,
        exc_type: type[BaseException] | None,
        exc_val: BaseException | None,
        exc_tb: Any,
    ) -> None:
        """Exit context manager - unload model.

        Cleanup happens regardless of exceptions in the with block.
        Does not suppress exceptions.
        """
        self.unload()
        # Return None to propagate exceptions
