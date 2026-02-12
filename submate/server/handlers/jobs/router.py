"""Jobs API router for Submate UI."""

import logging
import uuid
from pathlib import Path

from fastapi import APIRouter, HTTPException, Query, Response

from submate.config import get_config
from submate.database.models import Job
from submate.database.repository import ItemRepository, JobRepository, LibraryRepository, SubtitleRepository
from submate.database.session import get_db_session
from submate.server.handlers.jobs.models import (
    BulkTranscribeRequest,
    BulkTranscribeResponse,
    JobListResponse,
    JobResponse,
    TranscribeRequest,
    TranscribeResponse,
)
from submate.services.event_bus import get_event_bus

logger = logging.getLogger(__name__)


def _get_db_path() -> Path:
    """Get database path from configuration.

    Returns:
        Path to the SQLite database file.
    """
    config = get_config()
    return Path(config.queue.db_path)


def _job_to_response(job: Job, item_title: str) -> JobResponse:
    """Convert a database Job to a JobResponse.

    Args:
        job: The database Job model.
        item_title: Title of the associated item.

    Returns:
        JobResponse with job details.
    """
    return JobResponse(
        id=job.id,
        item_id=job.item_id,
        item_title=item_title,
        language=job.language,
        status=job.status,
        error=job.error,
        created_at=job.created_at,
        started_at=job.started_at,
        completed_at=job.completed_at,
    )


def create_jobs_router() -> APIRouter:
    """Create jobs API router.

    Returns:
        APIRouter with jobs endpoints.
    """
    router = APIRouter(prefix="/api", tags=["jobs"])

    @router.post("/items/{item_id}/transcribe", response_model=TranscribeResponse)
    async def transcribe_item(item_id: str, request: TranscribeRequest) -> TranscribeResponse:
        """Queue a transcription job for a single item.

        Args:
            item_id: The item ID to transcribe.
            request: TranscribeRequest with target language.

        Returns:
            TranscribeResponse with job ID.

        Raises:
            HTTPException: 404 if item not found.
        """
        db_path = _get_db_path()

        with get_db_session(db_path) as session:
            item_repo = ItemRepository(session)
            job_repo = JobRepository(session)

            # Verify item exists
            item = item_repo.get_by_id(item_id)
            if item is None:
                raise HTTPException(status_code=404, detail="Item not found")

            # Create job
            job_id = str(uuid.uuid4())
            job_repo.create(
                id=job_id,
                item_id=item_id,
                language=request.language,
                status="pending",
            )

        # Publish job.created event
        event_bus = get_event_bus()
        event_bus.publish("job.created", {"job_id": job_id, "item_id": item_id, "language": request.language})

        return TranscribeResponse(job_id=job_id, message="Transcription job queued")

    @router.post("/libraries/{library_id}/transcribe", response_model=BulkTranscribeResponse)
    async def transcribe_library(library_id: str, request: TranscribeRequest) -> BulkTranscribeResponse:
        """Queue transcription jobs for all items in library missing subtitles.

        Args:
            library_id: The library ID to transcribe.
            request: TranscribeRequest with target language.

        Returns:
            BulkTranscribeResponse with list of created jobs.

        Raises:
            HTTPException: 404 if library not found.
        """
        db_path = _get_db_path()
        jobs_created: list[TranscribeResponse] = []

        with get_db_session(db_path) as session:
            library_repo = LibraryRepository(session)
            item_repo = ItemRepository(session)
            subtitle_repo = SubtitleRepository(session)
            job_repo = JobRepository(session)

            # Verify library exists
            library = library_repo.get_by_id(library_id)
            if library is None:
                raise HTTPException(status_code=404, detail="Library not found")

            # Get all items in library
            items = item_repo.list_by_library(library_id, limit=10000, offset=0)

            # Filter items missing subtitle for target language
            event_bus = get_event_bus()
            for item in items:
                # Check if item already has subtitle in target language
                existing_subtitle = subtitle_repo.get_by_item_and_language(item.id, request.language)
                if existing_subtitle is not None:
                    continue

                # Create job for this item
                job_id = str(uuid.uuid4())
                job_repo.create(
                    id=job_id,
                    item_id=item.id,
                    language=request.language,
                    status="pending",
                )

                # Publish event
                event_bus.publish("job.created", {"job_id": job_id, "item_id": item.id, "language": request.language})

                jobs_created.append(TranscribeResponse(job_id=job_id, message="Transcription job queued"))

        return BulkTranscribeResponse(jobs=jobs_created, total_queued=len(jobs_created))

    @router.post("/bulk/transcribe", response_model=BulkTranscribeResponse)
    async def bulk_transcribe(request: BulkTranscribeRequest) -> BulkTranscribeResponse:
        """Queue transcription jobs for selected items.

        Args:
            request: BulkTranscribeRequest with item IDs and language.

        Returns:
            BulkTranscribeResponse with list of created jobs.
        """
        db_path = _get_db_path()
        jobs_created: list[TranscribeResponse] = []

        with get_db_session(db_path) as session:
            item_repo = ItemRepository(session)
            job_repo = JobRepository(session)

            event_bus = get_event_bus()
            for item_id in request.item_ids:
                # Verify item exists
                item = item_repo.get_by_id(item_id)
                if item is None:
                    # Skip non-existent items in bulk operation
                    continue

                # Create job
                job_id = str(uuid.uuid4())
                job_repo.create(
                    id=job_id,
                    item_id=item_id,
                    language=request.language,
                    status="pending",
                )

                # Publish event
                event_bus.publish("job.created", {"job_id": job_id, "item_id": item_id, "language": request.language})

                jobs_created.append(TranscribeResponse(job_id=job_id, message="Transcription job queued"))

        return BulkTranscribeResponse(jobs=jobs_created, total_queued=len(jobs_created))

    @router.get("/jobs", response_model=JobListResponse)
    async def list_jobs(
        page: int = Query(default=1, ge=1, description="Page number (1-indexed)"),
        page_size: int = Query(default=50, ge=1, le=100, description="Jobs per page (max 100)"),
        status: str | None = None,
    ) -> JobListResponse:
        """List jobs with filtering and pagination.

        Args:
            page: Page number (1-indexed).
            page_size: Number of jobs per page.
            status: Optional status filter.

        Returns:
            JobListResponse with paginated jobs.
        """
        db_path = _get_db_path()
        offset = (page - 1) * page_size

        with get_db_session(db_path) as session:
            item_repo = ItemRepository(session)

            # Build query
            query = session.query(Job)
            if status is not None:
                query = query.filter(Job.status == status)

            # Get total count
            total = query.count()

            # Get paginated jobs
            jobs = query.order_by(Job.created_at.desc()).offset(offset).limit(page_size).all()

            # Convert to responses with item titles
            job_responses = []
            for job in jobs:
                item = item_repo.get_by_id(job.item_id)
                item_title = item.title if item else "Unknown"
                job_responses.append(_job_to_response(job, item_title))

            return JobListResponse(
                jobs=job_responses,
                total=total,
                page=page,
                page_size=page_size,
            )

    @router.post("/jobs/{job_id}/retry", response_model=JobResponse)
    async def retry_job(job_id: str) -> JobResponse:
        """Retry a failed job.

        Args:
            job_id: The job ID to retry.

        Returns:
            JobResponse with updated job details.

        Raises:
            HTTPException: 404 if job not found.
            HTTPException: 400 if job is not in failed state.
        """
        db_path = _get_db_path()

        with get_db_session(db_path) as session:
            job_repo = JobRepository(session)
            item_repo = ItemRepository(session)

            job = job_repo.get_by_id(job_id)
            if job is None:
                raise HTTPException(status_code=404, detail="Job not found")

            if job.status != "failed":
                raise HTTPException(status_code=400, detail="Only failed jobs can be retried")

            # Reset job to pending state
            job.status = "pending"
            job.error = None
            job.started_at = None
            job.completed_at = None
            session.flush()

            # Get item title
            item = item_repo.get_by_id(job.item_id)
            item_title = item.title if item else "Unknown"

            # Publish job.created event for retry
            event_bus = get_event_bus()
            event_bus.publish("job.created", {"job_id": job_id, "item_id": job.item_id, "language": job.language})

            return _job_to_response(job, item_title)

    @router.delete("/jobs/{job_id}", status_code=204)
    async def cancel_job(job_id: str) -> Response:
        """Cancel a pending job.

        Args:
            job_id: The job ID to cancel.

        Returns:
            204 No Content on success.

        Raises:
            HTTPException: 404 if job not found.
            HTTPException: 400 if job is not in pending state.
        """
        db_path = _get_db_path()

        with get_db_session(db_path) as session:
            job_repo = JobRepository(session)

            job = job_repo.get_by_id(job_id)
            if job is None:
                raise HTTPException(status_code=404, detail="Job not found")

            if job.status != "pending":
                raise HTTPException(status_code=400, detail="Only pending jobs can be cancelled")

            # Delete the job
            session.delete(job)
            session.flush()

        return Response(status_code=204)

    return router
