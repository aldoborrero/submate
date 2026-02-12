import { useState, useEffect } from 'react'
import { useParams, Link } from 'react-router-dom'
import { itemsApi } from '@/api'
import type { Item, Subtitle } from '@/api'
import { SubtitleEditor } from '@/components/SubtitleEditor'

export function ItemDetailPage() {
  const { id } = useParams<{ id: string }>()
  const [item, setItem] = useState<Item | null>(null)
  const [subtitles, setSubtitles] = useState<Subtitle[]>([])
  const [selectedLanguage, setSelectedLanguage] = useState<string | null>(null)
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    async function fetchItem() {
      if (!id) return
      setLoading(true)
      try {
        const response = await itemsApi.getItem(id)
        setItem(response.item)
        setSubtitles(response.subtitles)
        if (response.subtitles.length > 0) {
          setSelectedLanguage(response.subtitles[0].language)
        }
      } catch (error) {
        console.error('Failed to fetch item:', error)
      } finally {
        setLoading(false)
      }
    }
    fetchItem()
  }, [id])

  const handleSave = async () => {
    if (!item) return
    // Refetch to update subtitle list
    try {
      const response = await itemsApi.getItem(item.id)
      setSubtitles(response.subtitles)
    } catch (error) {
      console.error('Failed to refresh subtitles:', error)
    }
  }

  if (loading) {
    return <div className="text-gray-400 text-center py-12">Loading...</div>
  }

  if (!item) {
    return <div className="text-red-400 text-center py-12">Item not found</div>
  }

  const backLink = item.type === 'movie' ? '/movies' : '/series'
  const backLabel = item.type === 'movie' ? 'Movies' : 'Series'

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-start gap-6">
        <img
          src={itemsApi.getPosterUrl(item.id)}
          alt={item.title}
          className="w-32 rounded-lg"
          onError={(e) => {
            ;(e.target as HTMLImageElement).src = '/placeholder.png'
          }}
        />
        <div className="flex-1">
          <Link to={backLink} className="text-primary-400 hover:underline text-sm">
            &larr; Back to {backLabel}
          </Link>
          <h1 className="text-2xl font-bold text-white mt-2">{item.title}</h1>
          {item.series_name && (
            <p className="text-gray-400">
              {item.series_name} - S{String(item.season_num).padStart(2, '0')}E
              {String(item.episode_num).padStart(2, '0')}
            </p>
          )}
        </div>
      </div>

      {/* Subtitle Tabs */}
      {subtitles.length > 0 ? (
        <div className="space-y-4">
          <div className="flex items-center gap-2">
            {subtitles.map((sub) => (
              <button
                key={sub.language}
                onClick={() => setSelectedLanguage(sub.language)}
                className={`
                  px-4 py-2 rounded-lg text-sm font-medium transition-colors
                  ${
                    selectedLanguage === sub.language
                      ? 'bg-primary-600 text-white'
                      : 'bg-gray-800 text-gray-300 hover:bg-gray-700'
                  }
                `}
              >
                {sub.language.toUpperCase()}
                <span className="ml-2 text-xs text-gray-400">({sub.source})</span>
              </button>
            ))}
          </div>

          {selectedLanguage && (
            <SubtitleEditor itemId={item.id} language={selectedLanguage} onSave={handleSave} />
          )}
        </div>
      ) : (
        <div className="text-center py-12 text-gray-400">No subtitles available for this item.</div>
      )}
    </div>
  )
}
