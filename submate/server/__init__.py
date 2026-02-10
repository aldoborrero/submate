"""Submate server module."""

from .handlers.bazarr.handlers import handle_asr_request, handle_detect_language
from .server import app

__all__ = ["app", "handle_asr_request", "handle_detect_language"]
