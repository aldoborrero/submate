/**
 * Fetch-based API client for Submate backend.
 *
 * All requests go through the Vite proxy (/api/* -> http://localhost:9000/api/*).
 */

import type {
  Library,
  LibraryListResponse,
  LibraryUpdateRequest,
  ItemListResponse,
  ItemDetailResponse,
  SeriesDetailResponse,
  Job,
  JobListResponse,
  TranscribeRequest,
  TranscribeResponse,
  BulkTranscribeRequest,
  BulkTranscribeResponse,
  Subtitle,
  SubtitleListResponse,
  SubtitleContentResponse,
  SubtitleUpdateRequest,
  SyncResponse,
  Settings,
  SettingsUpdateRequest,
  TestConnectionResponse,
  JellyfinSettings,
  NotificationSettings,
  SSEEvent,
} from './types'

const BASE_URL = '/api'

/**
 * Custom error class for API errors.
 */
export class ApiError extends Error {
  constructor(
    message: string,
    public status: number,
    public details?: unknown
  ) {
    super(message)
    this.name = 'ApiError'
  }
}

/**
 * Make an HTTP request to the API.
 *
 * @param endpoint - API endpoint (without /api prefix)
 * @param options - Fetch options
 * @returns Parsed JSON response
 * @throws ApiError on HTTP errors
 */
async function request<T>(
  endpoint: string,
  options: RequestInit = {}
): Promise<T> {
  const url = `${BASE_URL}${endpoint}`

  const headers: HeadersInit = {
    'Content-Type': 'application/json',
    ...options.headers,
  }

  const response = await fetch(url, {
    ...options,
    headers,
  })

  if (!response.ok) {
    let error: { detail?: string } = {}
    try {
      error = await response.json()
    } catch {
      // Response body is not JSON
    }
    throw new ApiError(
      error.detail || `Request failed: ${response.status}`,
      response.status,
      error
    )
  }

  // Handle 204 No Content and empty responses
  if (response.status === 204) {
    return {} as T
  }

  const text = await response.text()
  if (!text) {
    return {} as T
  }

  return JSON.parse(text) as T
}

// ============================================================================
// Libraries API
// ============================================================================

export const librariesApi = {
  /**
   * List all libraries.
   */
  list: (): Promise<LibraryListResponse> => request('/libraries'),

  /**
   * Get a single library by ID.
   */
  get: (id: string): Promise<Library> => request(`/libraries/${id}`),

  /**
   * Update library settings.
   */
  update: (id: string, data: LibraryUpdateRequest): Promise<Library> =>
    request(`/libraries/${id}`, {
      method: 'PATCH',
      body: JSON.stringify(data),
    }),

  /**
   * Sync libraries from Jellyfin.
   */
  sync: (): Promise<SyncResponse> =>
    request('/libraries/sync', {
      method: 'POST',
    }),
}

// ============================================================================
// Items API
// ============================================================================

export interface ListItemsParams {
  page?: number
  page_size?: number
  library_id?: string
}

export const itemsApi = {
  /**
   * List movies with pagination.
   */
  listMovies: (params?: ListItemsParams): Promise<ItemListResponse> => {
    const searchParams = new URLSearchParams()
    if (params?.page) searchParams.set('page', String(params.page))
    if (params?.page_size) searchParams.set('page_size', String(params.page_size))
    if (params?.library_id) searchParams.set('library_id', params.library_id)
    const query = searchParams.toString()
    return request(`/movies${query ? `?${query}` : ''}`)
  },

  /**
   * List series with pagination.
   */
  listSeries: (params?: ListItemsParams): Promise<ItemListResponse> => {
    const searchParams = new URLSearchParams()
    if (params?.page) searchParams.set('page', String(params.page))
    if (params?.page_size) searchParams.set('page_size', String(params.page_size))
    if (params?.library_id) searchParams.set('library_id', params.library_id)
    const query = searchParams.toString()
    return request(`/series${query ? `?${query}` : ''}`)
  },

  /**
   * Get series detail with episodes.
   */
  getSeries: (id: string): Promise<SeriesDetailResponse> => request(`/series/${id}`),

  /**
   * Get a single item by ID with its subtitles.
   */
  getItem: (id: string): Promise<ItemDetailResponse> => request(`/items/${id}`),

  /**
   * Get poster URL for an item.
   * Returns the URL string (not a request).
   */
  getPosterUrl: (id: string): string => `${BASE_URL}/items/${id}/poster`,
}

// ============================================================================
// Jobs API
// ============================================================================

export interface ListJobsParams {
  page?: number
  page_size?: number
  status?: string
}

export const jobsApi = {
  /**
   * List jobs with pagination and filtering.
   */
  list: (params?: ListJobsParams): Promise<JobListResponse> => {
    const searchParams = new URLSearchParams()
    if (params?.page) searchParams.set('page', String(params.page))
    if (params?.page_size) searchParams.set('page_size', String(params.page_size))
    if (params?.status) searchParams.set('status', params.status)
    const query = searchParams.toString()
    return request(`/jobs${query ? `?${query}` : ''}`)
  },

  /**
   * Queue a transcription job for a single item.
   */
  transcribeItem: (itemId: string, data: TranscribeRequest): Promise<TranscribeResponse> =>
    request(`/items/${itemId}/transcribe`, {
      method: 'POST',
      body: JSON.stringify(data),
    }),

  /**
   * Queue transcription jobs for all items in a library missing subtitles.
   */
  transcribeLibrary: (libraryId: string, data: TranscribeRequest): Promise<BulkTranscribeResponse> =>
    request(`/libraries/${libraryId}/transcribe`, {
      method: 'POST',
      body: JSON.stringify(data),
    }),

  /**
   * Queue transcription jobs for selected items.
   */
  transcribeBulk: (data: BulkTranscribeRequest): Promise<BulkTranscribeResponse> =>
    request('/bulk/transcribe', {
      method: 'POST',
      body: JSON.stringify(data),
    }),

  /**
   * Retry a failed job.
   */
  retry: (jobId: string): Promise<Job> =>
    request(`/jobs/${jobId}/retry`, { method: 'POST' }),

  /**
   * Cancel a pending job.
   */
  cancel: (jobId: string): Promise<void> =>
    request(`/jobs/${jobId}`, { method: 'DELETE' }),
}

// ============================================================================
// Subtitles API
// ============================================================================

export const subtitlesApi = {
  /**
   * List all subtitles for an item.
   */
  list: (itemId: string): Promise<SubtitleListResponse> =>
    request(`/items/${itemId}/subtitles`),

  /**
   * Get subtitle content for an item.
   */
  getContent: (itemId: string, language: string): Promise<SubtitleContentResponse> =>
    request(`/items/${itemId}/subtitles/${language}`),

  /**
   * Save or update subtitle content.
   */
  save: (itemId: string, language: string, data: SubtitleUpdateRequest): Promise<Subtitle> =>
    request(`/items/${itemId}/subtitles/${language}`, {
      method: 'PUT',
      body: JSON.stringify(data),
    }),

  /**
   * Delete a subtitle.
   */
  delete: (itemId: string, language: string): Promise<void> =>
    request(`/items/${itemId}/subtitles/${language}`, {
      method: 'DELETE',
    }),

  /**
   * Sync subtitle timing with ffsubsync.
   */
  sync: (itemId: string, language: string): Promise<SyncResponse> =>
    request(`/items/${itemId}/subtitles/${language}/sync`, {
      method: 'POST',
    }),
}

// ============================================================================
// Settings API
// ============================================================================

export const settingsApi = {
  /**
   * Get current settings.
   */
  get: (): Promise<Settings> => request('/settings'),

  /**
   * Update settings.
   */
  update: (data: SettingsUpdateRequest): Promise<Settings> =>
    request('/settings', {
      method: 'PUT',
      body: JSON.stringify(data),
    }),

  /**
   * Test Jellyfin connection with provided settings.
   */
  testJellyfin: (data: JellyfinSettings): Promise<TestConnectionResponse> =>
    request('/settings/test-jellyfin', {
      method: 'POST',
      body: JSON.stringify(data),
    }),

  /**
   * Test notification configuration.
   */
  testNotification: (data: NotificationSettings): Promise<TestConnectionResponse> =>
    request('/settings/test-notification', {
      method: 'POST',
      body: JSON.stringify(data),
    }),
}

// ============================================================================
// SSE Events
// ============================================================================

/**
 * Subscribe to server-sent events.
 *
 * @param onEvent - Callback for each event received
 * @param onError - Optional callback for errors
 * @returns Unsubscribe function to close the connection
 *
 * @example
 * ```typescript
 * const unsubscribe = subscribeToEvents(
 *   (event) => {
 *     console.log('Event:', event.event_type, event.data)
 *   },
 *   (error) => {
 *     console.error('SSE error:', error)
 *   }
 * )
 *
 * // Later, to cleanup:
 * unsubscribe()
 * ```
 */
export function subscribeToEvents(
  onEvent: (event: SSEEvent) => void,
  onError?: (error: Event) => void
): () => void {
  const eventSource = new EventSource(`${BASE_URL}/events`)

  // Handle named events (job.created, job.started, etc.)
  const eventTypes = [
    'job.created',
    'job.started',
    'job.completed',
    'job.failed',
    'sync.completed',
  ]

  eventTypes.forEach((eventType) => {
    eventSource.addEventListener(eventType, (event: MessageEvent) => {
      try {
        const data = JSON.parse(event.data)
        onEvent(data as SSEEvent)
      } catch (e) {
        console.error('Failed to parse SSE event:', e)
      }
    })
  })

  // Also handle generic message events (fallback)
  eventSource.onmessage = (event: MessageEvent) => {
    try {
      const data = JSON.parse(event.data)
      onEvent(data as SSEEvent)
    } catch (e) {
      console.error('Failed to parse SSE event:', e)
    }
  }

  eventSource.onerror = (error: Event) => {
    if (onError) {
      onError(error)
    }
  }

  // Return unsubscribe function
  return () => {
    eventSource.close()
  }
}
