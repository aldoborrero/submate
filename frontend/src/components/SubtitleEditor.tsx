import { useState, useEffect } from 'react'
import { subtitlesApi } from '@/api'

interface SubtitleEntry {
  index: number
  startTime: string
  endTime: string
  text: string
}

interface SubtitleEditorProps {
  itemId: string
  language: string
  onSave?: () => void
}

export function SubtitleEditor({ itemId, language, onSave }: SubtitleEditorProps) {
  const [entries, setEntries] = useState<SubtitleEntry[]>([])
  const [loading, setLoading] = useState(true)
  const [saving, setSaving] = useState(false)
  const [syncing, setSyncing] = useState(false)
  const [timeShift, setTimeShift] = useState(0)
  const [editingIndex, setEditingIndex] = useState<number | null>(null)

  useEffect(() => {
    async function fetchContent() {
      setLoading(true)
      try {
        const response = await subtitlesApi.getContent(itemId, language)
        setEntries(parseSrt(response.content))
      } catch (error) {
        console.error('Failed to fetch subtitle:', error)
      } finally {
        setLoading(false)
      }
    }
    fetchContent()
  }, [itemId, language])

  const handleSave = async () => {
    setSaving(true)
    try {
      const srtContent = entriesToSrt(entries)
      await subtitlesApi.save(itemId, language, { content: srtContent })
      onSave?.()
    } catch (error) {
      console.error('Failed to save subtitle:', error)
    } finally {
      setSaving(false)
    }
  }

  const handleSync = async () => {
    setSyncing(true)
    try {
      await subtitlesApi.sync(itemId, language)
      // Refetch after sync
      const response = await subtitlesApi.getContent(itemId, language)
      setEntries(parseSrt(response.content))
      setTimeShift(0)
    } catch (error) {
      console.error('Failed to sync subtitle:', error)
    } finally {
      setSyncing(false)
    }
  }

  const handleTimeShift = (ms: number) => {
    setEntries(
      entries.map((entry) => ({
        ...entry,
        startTime: shiftTime(entry.startTime, ms),
        endTime: shiftTime(entry.endTime, ms),
      }))
    )
    setTimeShift((prev) => prev + ms)
  }

  const handleEntryChange = (
    index: number,
    field: 'text' | 'startTime' | 'endTime',
    value: string
  ) => {
    setEntries(entries.map((entry, i) => (i === index ? { ...entry, [field]: value } : entry)))
  }

  if (loading) {
    return <div className="text-gray-400 py-4">Loading subtitle...</div>
  }

  return (
    <div className="space-y-4">
      {/* Toolbar */}
      <div className="flex items-center justify-between bg-gray-900 p-4 rounded-lg">
        <div className="flex items-center gap-4">
          {/* Time Shift Controls */}
          <div className="flex items-center gap-2">
            <span className="text-gray-400 text-sm">Time Shift:</span>
            <button
              onClick={() => handleTimeShift(-500)}
              className="px-2 py-1 bg-gray-800 hover:bg-gray-700 text-white rounded text-sm"
            >
              -500ms
            </button>
            <button
              onClick={() => handleTimeShift(-100)}
              className="px-2 py-1 bg-gray-800 hover:bg-gray-700 text-white rounded text-sm"
            >
              -100ms
            </button>
            <button
              onClick={() => handleTimeShift(100)}
              className="px-2 py-1 bg-gray-800 hover:bg-gray-700 text-white rounded text-sm"
            >
              +100ms
            </button>
            <button
              onClick={() => handleTimeShift(500)}
              className="px-2 py-1 bg-gray-800 hover:bg-gray-700 text-white rounded text-sm"
            >
              +500ms
            </button>
            {timeShift !== 0 && (
              <span className="text-primary-400 text-sm">
                ({timeShift > 0 ? '+' : ''}
                {timeShift}ms)
              </span>
            )}
          </div>

          {/* Sync Button */}
          <button
            onClick={handleSync}
            disabled={syncing}
            className="px-3 py-1 bg-blue-600 hover:bg-blue-700 text-white rounded text-sm disabled:opacity-50 disabled:cursor-not-allowed"
          >
            {syncing ? 'Syncing...' : 'Auto-sync (ffsubsync)'}
          </button>
        </div>

        {/* Save Button */}
        <button
          onClick={handleSave}
          disabled={saving}
          className={`
            px-4 py-2 rounded-lg font-medium transition-colors
            ${
              saving
                ? 'bg-gray-700 text-gray-400 cursor-not-allowed'
                : 'bg-primary-600 hover:bg-primary-700 text-white'
            }
          `}
        >
          {saving ? 'Saving...' : 'Save Changes'}
        </button>
      </div>

      {/* Subtitle Table */}
      <div className="bg-gray-900 rounded-lg overflow-hidden">
        <table className="w-full">
          <thead className="bg-gray-800">
            <tr>
              <th className="px-4 py-3 text-left text-xs font-medium text-gray-400 uppercase">
                #
              </th>
              <th className="px-4 py-3 text-left text-xs font-medium text-gray-400 uppercase">
                Start
              </th>
              <th className="px-4 py-3 text-left text-xs font-medium text-gray-400 uppercase">
                End
              </th>
              <th className="px-4 py-3 text-left text-xs font-medium text-gray-400 uppercase">
                Text
              </th>
            </tr>
          </thead>
          <tbody className="divide-y divide-gray-800">
            {entries.map((entry, index) => (
              <tr
                key={index}
                className="hover:bg-gray-800/50 cursor-pointer"
                onClick={() => setEditingIndex(index)}
              >
                <td className="px-4 py-3 text-gray-500 text-sm">{entry.index}</td>
                <td className="px-4 py-3">
                  {editingIndex === index ? (
                    <input
                      type="text"
                      value={entry.startTime}
                      onChange={(e) => handleEntryChange(index, 'startTime', e.target.value)}
                      onClick={(e) => e.stopPropagation()}
                      className="w-28 px-2 py-1 bg-gray-800 border border-gray-600 rounded text-white text-sm"
                    />
                  ) : (
                    <span className="text-gray-300 font-mono text-sm">{entry.startTime}</span>
                  )}
                </td>
                <td className="px-4 py-3">
                  {editingIndex === index ? (
                    <input
                      type="text"
                      value={entry.endTime}
                      onChange={(e) => handleEntryChange(index, 'endTime', e.target.value)}
                      onClick={(e) => e.stopPropagation()}
                      className="w-28 px-2 py-1 bg-gray-800 border border-gray-600 rounded text-white text-sm"
                    />
                  ) : (
                    <span className="text-gray-300 font-mono text-sm">{entry.endTime}</span>
                  )}
                </td>
                <td className="px-4 py-3">
                  {editingIndex === index ? (
                    <textarea
                      value={entry.text}
                      onChange={(e) => handleEntryChange(index, 'text', e.target.value)}
                      onClick={(e) => e.stopPropagation()}
                      rows={2}
                      className="w-full px-2 py-1 bg-gray-800 border border-gray-600 rounded text-white text-sm resize-none"
                    />
                  ) : (
                    <span className="text-white text-sm whitespace-pre-wrap">{entry.text}</span>
                  )}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      {entries.length === 0 && (
        <div className="text-center py-8 text-gray-400">No subtitle entries found</div>
      )}
    </div>
  )
}

// SRT parsing utilities

function parseSrt(content: string): SubtitleEntry[] {
  const entries: SubtitleEntry[] = []
  const blocks = content.trim().split(/\n\n+/)

  for (const block of blocks) {
    const lines = block.split('\n')
    if (lines.length < 3) continue

    const index = parseInt(lines[0], 10)
    if (isNaN(index)) continue

    const timeMatch = lines[1].match(
      /(\d{2}:\d{2}:\d{2},\d{3})\s*-->\s*(\d{2}:\d{2}:\d{2},\d{3})/
    )
    if (!timeMatch) continue

    const text = lines.slice(2).join('\n')

    entries.push({
      index,
      startTime: timeMatch[1],
      endTime: timeMatch[2],
      text,
    })
  }

  return entries
}

function entriesToSrt(entries: SubtitleEntry[]): string {
  return entries
    .map((entry, i) => `${i + 1}\n${entry.startTime} --> ${entry.endTime}\n${entry.text}`)
    .join('\n\n')
}

function shiftTime(time: string, ms: number): string {
  // Parse "00:01:23,456" format
  const match = time.match(/(\d{2}):(\d{2}):(\d{2}),(\d{3})/)
  if (!match) return time

  const [, hStr, mStr, sStr, millisStr] = match
  const h = parseInt(hStr, 10)
  const m = parseInt(mStr, 10)
  const s = parseInt(sStr, 10)
  const millis = parseInt(millisStr, 10)

  let totalMs = h * 3600000 + m * 60000 + s * 1000 + millis + ms

  // Ensure non-negative
  if (totalMs < 0) totalMs = 0

  const newH = Math.floor(totalMs / 3600000)
  const newM = Math.floor((totalMs % 3600000) / 60000)
  const newS = Math.floor((totalMs % 60000) / 1000)
  const newMs = totalMs % 1000

  return `${String(newH).padStart(2, '0')}:${String(newM).padStart(2, '0')}:${String(newS).padStart(2, '0')},${String(newMs).padStart(3, '0')}`
}
