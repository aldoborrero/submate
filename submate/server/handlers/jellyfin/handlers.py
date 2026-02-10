"""Jellyfin webhook handler."""

import logging

from submate.config import get_config
from submate.media_servers.jellyfin import JellyfinClient
from submate.paths import map_path
from submate.queue import get_task_queue
from submate.queue.tasks import TranscriptionTask
from submate.server.handlers.jellyfin.models import JellyfinWebhookPayload

logger = logging.getLogger(__name__)


async def handle_jellyfin_webhook(payload: JellyfinWebhookPayload) -> dict:
    """Handle incoming Jellyfin webhook event.

    Args:
        payload: Validated Jellyfin webhook payload

    Returns:
        Response dict with status and details
    """
    logger.info(f"Received Jellyfin webhook: {payload.notification_type}")

    # Check if we should process this event
    config = get_config()

    should_process = False
    if payload.is_item_added() and config.server.process_on_add:
        should_process = True
    elif payload.is_playback_start() and config.server.process_on_play:
        should_process = True

    if not should_process:
        logger.debug(f"Event {payload.notification_type} not configured for processing")
        return {
            "status": "skipped",
            "message": f"Event {payload.notification_type} not configured",
        }

    try:
        # Get file path from Jellyfin
        jellyfin = JellyfinClient(config)
        jellyfin.connect()
        file_path = jellyfin.get_file_path(payload.item_id)

        # Apply path mapping if configured (for Docker)
        mapped_path = map_path(
            file_path,
            use_mapping=config.path_mapping.enabled,
            path_from=config.path_mapping.from_path,
            path_to=config.path_mapping.to_path,
        )

        logger.info(f"Processing file: {mapped_path}")

        # Enqueue transcription task
        task_queue = get_task_queue()
        task_queue.enqueue(
            TranscriptionTask,
            file_path=mapped_path,
            language=None,  # Auto-detect
            force=False,
        )

        # Refresh Jellyfin metadata
        try:
            jellyfin.refresh_item(payload.item_id)
        except Exception as e:
            logger.warning(f"Failed to refresh Jellyfin metadata: {e}")

        return {
            "status": "queued",
            "task_id": payload.item_id,  # Use ItemId as task reference
            "file_path": mapped_path,
        }

    except Exception as e:
        logger.error(f"Failed to process Jellyfin webhook: {e}", exc_info=True)
        return {
            "status": "error",
            "message": str(e),
        }
