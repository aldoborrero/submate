import { useState, useEffect } from 'react'
import { useParams, Link } from 'react-router-dom'
import { itemsApi, jobsApi } from '@/api'
import type { Item, SeriesDetailResponse } from '@/api'
import { EpisodeList } from '@/components/EpisodeList'

export function SeriesDetailPage() {
  const { id } = useParams<{ id: string }>()
  const [series, setSeries] = useState<Item | null>(null)
  const [episodes, setEpisodes] = useState<Item[]>([])
  const [loading, setLoading] = useState(true)
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set())
  const [transcribing, setTranscribing] = useState(false)

  useEffect(() => {
    async function fetchSeries() {
      if (!id) return
      setLoading(true)
      try {
        const response: SeriesDetailResponse = await itemsApi.getSeries(id)
        setSeries(response)
        setEpisodes(response.episodes)
      } catch (error) {
        console.error('Failed to fetch series:', error)
      } finally {
        setLoading(false)
      }
    }
    fetchSeries()
  }, [id])

  const handleSelect = (episodeId: string) => {
    setSelectedIds((prev) => {
      const next = new Set(prev)
      if (next.has(episodeId)) {
        next.delete(episodeId)
      } else {
        next.add(episodeId)
      }
      return next
    })
  }

  const handleSelectAll = () => {
    if (selectedIds.size === episodes.length) {
      setSelectedIds(new Set())
    } else {
      setSelectedIds(new Set(episodes.map((e) => e.id)))
    }
  }

  const handleTranscribe = async () => {
    if (selectedIds.size === 0) return
    setTranscribing(true)
    try {
      await jobsApi.transcribeBulk({
        item_ids: Array.from(selectedIds),
        language: 'en',
      })
      setSelectedIds(new Set())
    } catch (error) {
      console.error('Failed to queue transcription:', error)
    } finally {
      setTranscribing(false)
    }
  }

  if (loading) {
    return <div className="text-gray-400 text-center py-12">Loading...</div>
  }

  if (!series) {
    return <div className="text-gray-400 text-center py-12">Series not found</div>
  }

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-start gap-6">
        {/* Poster */}
        <img
          src={itemsApi.getPosterUrl(series.id)}
          alt={series.title}
          className="w-48 h-auto rounded-lg"
          onError={(e) => {
            ;(e.target as HTMLImageElement).src = '/placeholder.png'
          }}
        />

        {/* Info */}
        <div className="flex-1">
          <Link to="/series" className="text-primary-400 hover:underline text-sm">
            &larr; Back to Series
          </Link>
          <h1 className="text-3xl font-bold text-white mt-2">{series.title}</h1>
          <p className="text-gray-400 mt-2">{episodes.length} episodes</p>

          {/* Actions */}
          <div className="flex items-center gap-4 mt-4">
            <button
              onClick={handleSelectAll}
              className="px-4 py-2 bg-gray-800 hover:bg-gray-700
                         text-white rounded-lg transition-colors"
            >
              {selectedIds.size === episodes.length ? 'Deselect All' : 'Select All'}
            </button>

            {selectedIds.size > 0 && (
              <button
                onClick={handleTranscribe}
                disabled={transcribing}
                className={`
                  px-4 py-2 rounded-lg font-medium transition-colors
                  ${
                    transcribing
                      ? 'bg-gray-700 text-gray-400 cursor-not-allowed'
                      : 'bg-primary-600 hover:bg-primary-700 text-white'
                  }
                `}
              >
                {transcribing ? 'Queuing...' : `Transcribe ${selectedIds.size} episodes`}
              </button>
            )}
          </div>
        </div>
      </div>

      {/* Episodes */}
      <EpisodeList episodes={episodes} selectedIds={selectedIds} onSelect={handleSelect} />
    </div>
  )
}
