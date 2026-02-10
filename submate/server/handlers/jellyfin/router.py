# submate/webhooks/jellyfin/router.py
"""Jellyfin webhook router factory for modular server composition."""

import logging

from fastapi import APIRouter, Body, Header, HTTPException

from submate.config import Config

from .handlers import handle_jellyfin_webhook
from .models import JellyfinWebhookPayload

logger = logging.getLogger(__name__)


def create_jellyfin_router(config: Config) -> APIRouter:
    """Create Jellyfin webhook router with all endpoints.

    Args:
        config: Application configuration

    Returns:
        APIRouter with Jellyfin endpoints
    """
    router = APIRouter(prefix="/webhooks", tags=["jellyfin"])

    @router.post("/jellyfin")
    async def jellyfin_webhook(
        user_agent: str = Header(None),
        payload: JellyfinWebhookPayload = Body(...),
    ) -> dict:
        """Handle Jellyfin webhook events.

        Accepts JSON webhooks from Jellyfin server when media is added or played.

        Configure in Jellyfin:
        1. Dashboard → Plugins → Webhook
        2. Add webhook: http://your-server:9000/webhooks/jellyfin
        3. Enable "Item Added" notifications
        """
        # Validate it's from Jellyfin
        if not user_agent or "Jellyfin-Server" not in user_agent:
            raise HTTPException(
                status_code=400,
                detail="Invalid request - not from Jellyfin server",
            )

        try:
            result = await handle_jellyfin_webhook(payload)
            return result

        except Exception as e:
            logger.error("Error processing Jellyfin webhook: %s", e, exc_info=True)
            raise HTTPException(status_code=500, detail=str(e))

    return router
