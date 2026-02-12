import { useState, useEffect } from 'react'
import { Link } from 'react-router-dom'
import { ColumnDef } from '@tanstack/react-table'
import { MoreHorizontal, Check, AlertTriangle, Loader2 } from 'lucide-react'
import { itemsApi, jobsApi } from '@/api'
import type { Item } from '@/api'
import { DataTable } from '@/components/DataTable'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { Checkbox } from '@/components/ui/checkbox'
import { Skeleton } from '@/components/ui/skeleton'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'

export function MoviesPage() {
  const [movies, setMovies] = useState<Item[]>([])
  const [total, setTotal] = useState(0)
  const [loading, setLoading] = useState(true)
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set())
  const [transcribing, setTranscribing] = useState(false)

  useEffect(() => {
    async function fetchMovies() {
      setLoading(true)
      try {
        const response = await itemsApi.listMovies({ page_size: 100 })
        setMovies(response.items)
        setTotal(response.total)
      } catch (error) {
        console.error('Failed to fetch movies:', error)
      } finally {
        setLoading(false)
      }
    }
    fetchMovies()
  }, [])

  const handleTranscribe = async (ids: string[]) => {
    setTranscribing(true)
    try {
      await jobsApi.transcribeBulk({ item_ids: ids, language: 'en' })
      setSelectedIds(new Set())
    } catch (error) {
      console.error('Failed to queue transcription:', error)
    } finally {
      setTranscribing(false)
    }
  }

  const columns: ColumnDef<Item>[] = [
    {
      id: "select",
      header: ({ table }) => (
        <Checkbox
          checked={table.getIsAllPageRowsSelected()}
          onChange={(e) => table.toggleAllPageRowsSelected(e.currentTarget.checked)}
          aria-label="Select all"
        />
      ),
      cell: ({ row }) => (
        <Checkbox
          checked={row.getIsSelected()}
          onChange={(e) => row.toggleSelected(e.currentTarget.checked)}
          aria-label="Select row"
        />
      ),
      enableSorting: false,
    },
    {
      accessorKey: "title",
      header: "Title",
      cell: ({ row }) => (
        <Link
          to={`/item/${row.original.id}`}
          className="font-medium hover:text-primary transition-colors"
        >
          {row.getValue("title")}
        </Link>
      ),
    },
    {
      accessorKey: "subtitle_languages",
      header: "Subtitles",
      cell: ({ row }) => {
        const languages = row.original.subtitle_languages || []
        if (languages.length === 0) {
          return <span className="text-muted-foreground">—</span>
        }
        return (
          <div className="flex gap-1 flex-wrap">
            {languages.slice(0, 3).map((lang) => (
              <Badge key={lang} variant="secondary" className="text-xs">
                {lang}
              </Badge>
            ))}
            {languages.length > 3 && (
              <Badge variant="outline" className="text-xs">
                +{languages.length - 3}
              </Badge>
            )}
          </div>
        )
      },
      enableSorting: false,
    },
    {
      id: "status",
      header: "Status",
      cell: ({ row }) => {
        const hasSubtitles = (row.original.subtitle_languages?.length || 0) > 0
        if (hasSubtitles) {
          return (
            <Badge variant="success" className="gap-1">
              <Check className="h-3 w-3" />
              Ready
            </Badge>
          )
        }
        return (
          <Badge variant="warning" className="gap-1">
            <AlertTriangle className="h-3 w-3" />
            Missing
          </Badge>
        )
      },
    },
    {
      id: "actions",
      cell: ({ row }) => (
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button variant="ghost" size="icon" className="h-8 w-8">
              <MoreHorizontal className="h-4 w-4" />
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end">
            <DropdownMenuItem onClick={() => handleTranscribe([row.original.id])}>
              Transcribe
            </DropdownMenuItem>
            <DropdownMenuItem asChild>
              <Link to={`/item/${row.original.id}`}>View Details</Link>
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      ),
    },
  ]

  if (loading) {
    return (
      <div className="space-y-6">
        <div>
          <Skeleton className="h-8 w-32" />
          <Skeleton className="h-4 w-48 mt-2" />
        </div>
        <Skeleton className="h-[400px]" />
      </div>
    )
  }

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold">Movies</h1>
          <p className="text-muted-foreground mt-1">{total} movies in your library</p>
        </div>

        {selectedIds.size > 0 && (
          <Button onClick={() => handleTranscribe(Array.from(selectedIds))} disabled={transcribing}>
            {transcribing ? (
              <>
                <Loader2 className="h-4 w-4 mr-2 animate-spin" />
                Queuing...
              </>
            ) : (
              `Transcribe ${selectedIds.size} selected`
            )}
          </Button>
        )}
      </div>

      {/* Table */}
      <DataTable
        columns={columns}
        data={movies}
        searchKey="title"
        searchPlaceholder="Search movies..."
      />
    </div>
  )
}
