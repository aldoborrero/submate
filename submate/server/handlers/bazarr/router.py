# submate/webhooks/bazarr/router.py
"""Bazarr ASR router factory for modular server composition."""

from collections.abc import Iterator
from io import BytesIO

from fastapi import APIRouter, File, HTTPException, Query, UploadFile
from fastapi.responses import StreamingResponse

from submate.config import Config

from .handlers import handle_asr_request, handle_detect_language
from .models import LanguageDetectionResponse


def create_bazarr_router(config: Config) -> APIRouter:
    """Create Bazarr ASR router with all endpoints.

    Args:
        config: Application configuration

    Returns:
        APIRouter with Bazarr endpoints
    """
    router = APIRouter(prefix="/bazarr", tags=["bazarr"])

    @router.post("/asr")
    async def asr_endpoint(
        task: str = Query(default="transcribe", pattern="^(transcribe|translate)$"),
        language: str | None = Query(default=None),
        output: str = Query(default="srt", pattern="^(srt|vtt|txt|json)$"),
        encode: bool = Query(default=True),
        word_timestamps: bool = Query(default=False),
        video_file: str | None = Query(default=None),
        audio_file: UploadFile = File(...),
    ) -> StreamingResponse:
        """Bazarr ASR endpoint for on-demand transcription.

        Accepts audio file uploads and returns subtitles immediately.
        Used by Bazarr's Whisper provider.

        Configure in Bazarr:
        1. Settings → Subtitles → Whisper Provider
        2. Endpoint: http://your-server:9000/bazarr/asr
        """
        try:
            # Read uploaded file
            audio_content = await audio_file.read()
            audio_bytes = BytesIO(audio_content)

            # Handle transcription
            result = await handle_asr_request(
                audio_file=audio_bytes,
                task=task,
                language=language,
                output=output,
                encode=encode,
                word_timestamps=word_timestamps,
                video_file=video_file,
            )

            # Stream response
            def generate() -> Iterator[str]:
                yield result

            return StreamingResponse(
                content=generate(),
                media_type="text/plain",
                headers={"Source": "Transcribed using stable-ts from Submate"},
            )

        except ValueError as e:
            raise HTTPException(status_code=400, detail=str(e))
        except Exception:
            raise HTTPException(status_code=500, detail="Transcription failed")
        finally:
            await audio_file.close()

    @router.post("/detect-language")
    async def detect_language_endpoint(
        encode: bool = Query(default=True),
        detect_lang_length: int = Query(default=30, ge=1, le=300),
        detect_lang_offset: int = Query(default=0, ge=0),
        video_file: str | None = Query(default=None),
        audio_file: UploadFile = File(...),
    ) -> LanguageDetectionResponse:
        """Bazarr language detection endpoint.

        Detects language from uploaded audio segment.
        Used by Bazarr for automatic language detection.

        Configure in Bazarr:
        1. Settings → Subtitles → Whisper Provider
        2. Language detection: Enabled
        """
        try:
            # Read uploaded file
            audio_content = await audio_file.read()
            audio_bytes = BytesIO(audio_content)

            # Handle language detection
            result = await handle_detect_language(
                audio_file=audio_bytes,
                offset=detect_lang_offset,
                length=detect_lang_length,
                video_file=video_file,
            )

            return result

        except Exception:
            # Return unknown instead of error (Bazarr compatible)
            return LanguageDetectionResponse(
                detected_language="Unknown",
                language_code="und",
            )
        finally:
            await audio_file.close()

    return router
