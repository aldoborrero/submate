"""FastAPI webhook server with modular router composition."""

import logging
from collections.abc import AsyncIterator
from contextlib import asynccontextmanager

from fastapi import FastAPI, Request
from fastapi.responses import JSONResponse

from submate import __version__
from submate.config import get_config

logger = logging.getLogger(__name__)


@asynccontextmanager
async def lifespan(app: FastAPI) -> AsyncIterator[None]:
    """Lifespan context manager for startup/shutdown."""
    logger.info("Starting Submate server")
    yield
    logger.info("Shutting down Submate server")


def create_app() -> FastAPI:
    """Create FastAPI application with configured feature routers.

    Features are enabled/disabled based on configuration settings.
    """
    config = get_config()

    app = FastAPI(
        title="Submate Server",
        description="AI-powered subtitle generation",
        version=__version__,
        lifespan=lifespan,
    )

    # Include core router (always enabled)
    try:
        from submate.server.handlers.core.router import create_core_router

        app.include_router(create_core_router())
        logger.info("Core router loaded")
    except Exception as e:
        logger.warning("Could not load core router: %s", e)

    # Include feature routers based on configuration
    if config.server.bazarr_enabled:
        try:
            from submate.server.handlers.bazarr.router import create_bazarr_router

            app.include_router(create_bazarr_router(config))
            logger.info("Bazarr integration enabled")
        except Exception as e:
            logger.warning("Could not load Bazarr router: %s", e)
    else:
        logger.info("Bazarr integration disabled")

    if config.server.jellyfin_enabled:
        try:
            from submate.server.handlers.jellyfin.router import create_jellyfin_router

            app.include_router(create_jellyfin_router(config))
            logger.info("Jellyfin integration enabled")
        except Exception as e:
            logger.warning("Could not load Jellyfin router: %s", e)
    else:
        logger.info("Jellyfin integration disabled")

    # Include library API router (always enabled for UI)
    try:
        from submate.server.handlers.library.router import create_library_router

        app.include_router(create_library_router())
        logger.info("Library API router loaded")
    except Exception as e:
        logger.warning("Could not load Library router: %s", e)

    # Include items API router (always enabled for UI)
    try:
        from submate.server.handlers.items.router import create_items_router

        app.include_router(create_items_router())
        logger.info("Items API router loaded")
    except Exception as e:
        logger.warning("Could not load Items router: %s", e)

    # Include jobs API router (always enabled for UI)
    try:
        from submate.server.handlers.jobs.router import create_jobs_router

        app.include_router(create_jobs_router())
        logger.info("Jobs API router loaded")
    except Exception as e:
        logger.warning("Could not load Jobs router: %s", e)

    # Include events API router (always enabled for UI)
    try:
        from submate.server.handlers.events.router import create_events_router

        app.include_router(create_events_router())
        logger.info("Events API router loaded")
    except Exception as e:
        logger.warning("Could not load Events router: %s", e)

    # Include subtitles API router (always enabled for UI)
    try:
        from submate.server.handlers.subtitles.router import create_subtitles_router

        app.include_router(create_subtitles_router())
        logger.info("Subtitles API router loaded")
    except Exception as e:
        logger.warning("Could not load Subtitles router: %s", e)

    # Include settings API router (always enabled for UI)
    try:
        from submate.server.handlers.settings.router import create_settings_router

        app.include_router(create_settings_router())
        logger.info("Settings API router loaded")
    except Exception as e:
        logger.warning("Could not load Settings router: %s", e)

    # Global exception handler
    @app.exception_handler(Exception)
    async def global_exception_handler(request: Request, exc: Exception) -> JSONResponse:
        """Global exception handler."""
        logger.error("Unhandled exception: %s", exc, exc_info=True)
        return JSONResponse(
            status_code=500,
            content={"detail": "Internal server error"},
        )

    return app


# Create global app instance
app = create_app()
