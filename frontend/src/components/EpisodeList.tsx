import { Link } from 'react-router-dom'
import type { Item } from '@/api'

interface EpisodeListProps {
  episodes: Item[]
  selectedIds: Set<string>
  onSelect: (id: string) => void
}

export function EpisodeList({ episodes, selectedIds, onSelect }: EpisodeListProps) {
  // Group episodes by season
  const episodesBySeason = episodes.reduce(
    (acc, ep) => {
      const season = ep.season_num ?? 0
      if (!acc[season]) acc[season] = []
      acc[season].push(ep)
      return acc
    },
    {} as Record<number, Item[]>
  )

  return (
    <div className="space-y-6">
      {Object.entries(episodesBySeason)
        .sort(([a], [b]) => Number(a) - Number(b))
        .map(([season, eps]) => (
          <div key={season}>
            <h3 className="text-lg font-semibold text-white mb-3">Season {season}</h3>
            <div className="space-y-2">
              {eps
                .sort((a, b) => (a.episode_num ?? 0) - (b.episode_num ?? 0))
                .map((ep) => (
                  <div
                    key={ep.id}
                    onClick={() => onSelect(ep.id)}
                    className={`
                      flex items-center gap-4 p-3 rounded-lg cursor-pointer
                      transition-colors
                      ${
                        selectedIds.has(ep.id)
                          ? 'bg-primary-900/50 border border-primary-500'
                          : 'bg-gray-800 hover:bg-gray-750 border border-transparent'
                      }
                    `}
                  >
                    <input
                      type="checkbox"
                      checked={selectedIds.has(ep.id)}
                      onChange={() => onSelect(ep.id)}
                      className="rounded"
                    />
                    <span className="text-gray-400 font-mono text-sm w-12">
                      E{String(ep.episode_num).padStart(2, '0')}
                    </span>
                    <Link
                      to={`/item/${ep.id}`}
                      className="text-white flex-1 hover:text-primary-400 transition-colors"
                      onClick={(e) => e.stopPropagation()}
                    >
                      {ep.title}
                    </Link>
                    <span
                      className={`
                      text-xs px-2 py-1 rounded
                      ${
                        (ep.subtitle_languages?.length ?? 0) > 0
                          ? 'bg-green-900 text-green-400'
                          : 'bg-gray-700 text-gray-400'
                      }
                    `}
                    >
                      {ep.subtitle_languages?.length ?? 0} subs
                    </span>
                  </div>
                ))}
            </div>
          </div>
        ))}
    </div>
  )
}
