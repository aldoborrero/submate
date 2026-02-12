"""Pydantic models for Jobs API endpoints."""

from datetime import datetime

from pydantic import BaseModel, Field


class JobResponse(BaseModel):
    """Response model for a single transcription job."""

    id: str = Field(description="Job ID")
    item_id: str = Field(description="Parent item ID")
    item_title: str = Field(description="Title of the associated item")
    language: str = Field(description="Target language code")
    status: str = Field(description="Job status ('pending', 'running', 'completed', 'failed')")
    error: str | None = Field(default=None, description="Error message if job failed")
    created_at: datetime = Field(description="When the job was created")
    started_at: datetime | None = Field(default=None, description="When the job started")
    completed_at: datetime | None = Field(default=None, description="When the job completed")


class JobListResponse(BaseModel):
    """Response model for listing jobs with pagination."""

    jobs: list[JobResponse] = Field(description="List of jobs")
    total: int = Field(description="Total number of jobs")
    page: int = Field(description="Current page number")
    page_size: int = Field(description="Number of jobs per page")


class TranscribeRequest(BaseModel):
    """Request model for queueing a transcription job."""

    language: str = Field(description="Target language code for transcription")


class BulkTranscribeRequest(BaseModel):
    """Request model for queueing bulk transcription jobs."""

    item_ids: list[str] = Field(description="List of item IDs to transcribe")
    language: str = Field(description="Target language code for transcription")


class TranscribeResponse(BaseModel):
    """Response model for a queued transcription job."""

    job_id: str = Field(description="ID of the created job")
    message: str = Field(description="Status message")


class BulkTranscribeResponse(BaseModel):
    """Response model for bulk transcription jobs."""

    jobs: list[TranscribeResponse] = Field(description="List of created jobs")
    total_queued: int = Field(description="Total number of jobs queued")
