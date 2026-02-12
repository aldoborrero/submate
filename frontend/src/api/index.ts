/**
 * Submate API Client
 *
 * This module exports all types and the API client for interacting
 * with the Submate backend.
 *
 * @example
 * ```typescript
 * import {
 *   librariesApi,
 *   itemsApi,
 *   jobsApi,
 *   subtitlesApi,
 *   settingsApi,
 *   subscribeToEvents,
 *   ApiError,
 *   type Library,
 *   type Job,
 * } from '@/api'
 *
 * // Fetch libraries
 * const { libraries } = await librariesApi.list()
 *
 * // Queue a transcription job
 * const job = await jobsApi.transcribeItem('item-id', { language: 'en' })
 *
 * // Subscribe to real-time events
 * const unsubscribe = subscribeToEvents((event) => {
 *   if (event.event_type === 'job.completed') {
 *     console.log('Job completed:', event.data)
 *   }
 * })
 * ```
 */

export * from './types'
export * from './client'
