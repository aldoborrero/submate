/**
 * TypeScript type definitions for Submate API.
 *
 * These types match the backend Pydantic models defined in:
 * - submate/server/handlers/library/models.py
 * - submate/server/handlers/items/models.py
 * - submate/server/handlers/jobs/models.py
 * - submate/server/handlers/subtitles/models.py
 * - submate/server/handlers/settings/models.py
 */

// ============================================================================
// Library Types
// ============================================================================

/**
 * A media library synced from Jellyfin.
 */
export interface Library {
  id: string
  name: string
  type: 'movies' | 'series'
  target_languages: string[]
  skip_existing: boolean
  enabled: boolean
  last_synced: string | null
  item_count: number
}

/**
 * Response for listing libraries.
 */
export interface LibraryListResponse {
  libraries: Library[]
  total: number
}

/**
 * Request for updating library settings.
 */
export interface LibraryUpdateRequest {
  target_languages?: string[]
  skip_existing?: boolean
  enabled?: boolean
}

// ============================================================================
// Item Types
// ============================================================================

/**
 * A media item (movie, series, or episode).
 */
export interface Item {
  id: string
  library_id: string
  type: 'movie' | 'series' | 'episode'
  title: string
  path: string
  series_id?: string | null
  series_name?: string | null
  season_num?: number | null
  episode_num?: number | null
  poster_url?: string | null
  last_synced: string
  subtitle_languages: string[]
}

/**
 * Response for listing items with pagination.
 */
export interface ItemListResponse {
  items: Item[]
  total: number
  page: number
  page_size: number
}

/**
 * Response for series detail with episodes.
 */
export interface SeriesDetailResponse extends Item {
  episodes: Item[]
  season_count: number
  episode_count: number
}

/**
 * Response for item detail with subtitles.
 */
export interface ItemDetailResponse {
  item: Item
  subtitles: Subtitle[]
}

// ============================================================================
// Job Types
// ============================================================================

/**
 * Status of a transcription job.
 */
export type JobStatus = 'pending' | 'running' | 'completed' | 'failed'

/**
 * A transcription job.
 */
export interface Job {
  id: string
  item_id: string
  item_title: string
  language: string
  status: JobStatus
  error?: string | null
  created_at: string
  started_at?: string | null
  completed_at?: string | null
}

/**
 * Response for listing jobs with pagination.
 */
export interface JobListResponse {
  jobs: Job[]
  total: number
  page: number
  page_size: number
}

/**
 * Request for queueing a transcription job.
 */
export interface TranscribeRequest {
  language: string
}

/**
 * Response for a queued transcription job.
 */
export interface TranscribeResponse {
  job_id: string
  message: string
}

/**
 * Request for bulk transcription jobs.
 */
export interface BulkTranscribeRequest {
  item_ids: string[]
  language: string
}

/**
 * Response for bulk transcription jobs.
 */
export interface BulkTranscribeResponse {
  jobs: TranscribeResponse[]
  total_queued: number
}

// ============================================================================
// Subtitle Types
// ============================================================================

/**
 * Source of a subtitle file.
 */
export type SubtitleSource = 'external' | 'generated'

/**
 * A subtitle file associated with an item.
 */
export interface Subtitle {
  id: number
  item_id: string
  language: string
  source: SubtitleSource
  path: string
  created_at: string
}

/**
 * Response for listing subtitles.
 */
export interface SubtitleListResponse {
  subtitles: Subtitle[]
  total: number
}

/**
 * Response for getting subtitle content.
 */
export interface SubtitleContentResponse {
  language: string
  content: string
  format: 'srt' | 'ass' | 'vtt' | 'unknown'
}

/**
 * Request for updating subtitle content.
 */
export interface SubtitleUpdateRequest {
  content: string
}

/**
 * Response for subtitle sync operation.
 */
export interface SyncResponse {
  success: boolean
  message: string
}

// ============================================================================
// Settings Types
// ============================================================================

/**
 * Jellyfin connection settings.
 */
export interface JellyfinSettings {
  server_url: string
  api_key: string
}

/**
 * Whisper transcription settings.
 */
export interface WhisperSettings {
  model: string
  device: string
  compute_type: string
}

/**
 * LLM translation settings.
 */
export interface TranslationSettings {
  backend: string
  ollama_url: string
  ollama_model: string
  openai_api_key: string
  openai_model: string
  anthropic_api_key: string
  claude_model: string
  gemini_api_key: string
  gemini_model: string
}

/**
 * Notification settings.
 */
export interface NotificationSettings {
  webhook_url: string | null
  ntfy_url: string | null
  ntfy_topic: string | null
  apprise_urls: string[]
}

/**
 * Complete application settings.
 */
export interface Settings {
  jellyfin: JellyfinSettings
  whisper: WhisperSettings
  translation: TranslationSettings
  notifications: NotificationSettings
}

/**
 * Request for updating settings.
 */
export interface SettingsUpdateRequest {
  jellyfin?: JellyfinSettings
  whisper?: WhisperSettings
  translation?: TranslationSettings
  notifications?: NotificationSettings
}

/**
 * Response for connection test endpoints.
 */
export interface TestConnectionResponse {
  success: boolean
  message: string
  details: Record<string, unknown>
}

// ============================================================================
// Event Types (SSE)
// ============================================================================

/**
 * Types of events streamed via SSE.
 */
export type EventType =
  | 'job.created'
  | 'job.started'
  | 'job.completed'
  | 'job.failed'
  | 'sync.completed'

/**
 * SSE event payload.
 */
export interface SSEEvent {
  event_type: EventType
  data: Record<string, unknown>
}
