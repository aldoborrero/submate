"""Bazarr ASR integration."""

from submate.server.handlers.bazarr.handlers import handle_asr_request, handle_detect_language
from submate.server.handlers.bazarr.models import LanguageDetectionResponse

__all__ = ["handle_asr_request", "handle_detect_language", "LanguageDetectionResponse"]
