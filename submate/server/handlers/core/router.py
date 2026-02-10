# submate/webhooks/core/router.py
"""Core server router with status and queue endpoints."""

from fastapi import APIRouter

from submate import __version__
from submate.queue import get_task_queue


def create_core_router() -> APIRouter:
    """Create core router with status and queue endpoints.

    Returns:
        APIRouter with core endpoints
    """
    router = APIRouter(tags=["core"])

    @router.get("/")
    async def root() -> dict:
        """Root endpoint with server info."""
        return {
            "name": "Submate Server",
            "version": __version__,
            "docs": "/docs",
            "endpoints": {
                "bazarr_asr": "/bazarr/asr",
                "bazarr_detect_language": "/bazarr/detect-language",
                "jellyfin": "/webhooks/jellyfin",
                "status": "/status",
                "queue": "/queue",
            },
        }

    @router.get("/status")
    async def status() -> dict:
        """Server health and status."""
        task_queue = get_task_queue()
        return {
            "status": "ok",
            "version": __version__,
            "queue": task_queue.stats,
        }

    @router.get("/queue")
    async def queue_status() -> dict:
        """Get queue statistics."""
        task_queue = get_task_queue()
        return task_queue.stats

    return router
