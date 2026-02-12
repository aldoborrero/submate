"""Jobs API router for Submate UI."""

import logging
import uuid

from fastapi import APIRouter, HTTPException, Query, Response

from submate.database.models import Job
from submate.server.dependencies import DbSession, ItemRepo, JobRepo, LibraryRepo, SubtitleRepo
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
    async def transcribe_item(
        item_id: str,
        request: TranscribeRequest,
        item_repo: ItemRepo,
        job_repo: JobRepo,
    ) -> TranscribeResponse:
        """Queue a transcription job for a single item.

        Raises:
            HTTPException: 404 if item not found.
        """
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
    async def transcribe_library(
        library_id: str,
        request: TranscribeRequest,
        library_repo: LibraryRepo,
        item_repo: ItemRepo,
        subtitle_repo: SubtitleRepo,
        job_repo: JobRepo,
    ) -> BulkTranscribeResponse:
        """Queue transcription jobs for all items in library missing subtitles.

        Raises:
            HTTPException: 404 if library not found.
        """
        jobs_created: list[TranscribeResponse] = []

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
    async def bulk_transcribe(
        request: BulkTranscribeRequest,
        item_repo: ItemRepo,
        job_repo: JobRepo,
    ) -> BulkTranscribeResponse:
        """Queue transcription jobs for selected items."""
        jobs_created: list[TranscribeResponse] = []

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
        session: DbSession,
        item_repo: ItemRepo,
        page: int = Query(default=1, ge=1, description="Page number (1-indexed)"),
        page_size: int = Query(default=50, ge=1, le=100, description="Jobs per page (max 100)"),
        status: str | None = None,
    ) -> JobListResponse:
        """List jobs with filtering and pagination."""
        offset = (page - 1) * page_size

        # Build query
        query = session.query(Job)
        if status is not None:
            query = query.filter(Job.status == status)

        # Get total count
        total = query.count()

        # Get paginated jobs
        jobs = query.order_by(Job.created_at.desc()).offset(offset).limit(page_size).all()

        # Convert to responses with item titles
        job_responses = [
            _job_to_response(job, item.title if (item := item_repo.get_by_id(job.item_id)) else "Unknown")
            for job in jobs
        ]

        return JobListResponse(
            jobs=job_responses,
            total=total,
            page=page,
            page_size=page_size,
        )

    @router.post("/jobs/{job_id}/retry", response_model=JobResponse)
    async def retry_job(
        job_id: str,
        session: DbSession,
        job_repo: JobRepo,
        item_repo: ItemRepo,
    ) -> JobResponse:
        """Retry a failed job.

        Raises:
            HTTPException: 404 if job not found.
            HTTPException: 400 if job is not in failed state.
        """
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
    async def cancel_job(
        job_id: str,
        session: DbSession,
        job_repo: JobRepo,
    ) -> Response:
        """Cancel a pending job.

        Raises:
            HTTPException: 404 if job not found.
            HTTPException: 400 if job is not in pending state.
        """
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
