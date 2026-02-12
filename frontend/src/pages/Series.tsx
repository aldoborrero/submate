import { useState, useEffect } from 'react'
import { Link } from 'react-router-dom'
import { ColumnDef } from '@tanstack/react-table'
import { MoreHorizontal, Check, AlertTriangle } from 'lucide-react'
import { itemsApi } from '@/api'
import type { Item } from '@/api'
import { DataTable } from '@/components/DataTable'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { Skeleton } from '@/components/ui/skeleton'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'

export function SeriesPage() {
  const [series, setSeries] = useState<Item[]>([])
  const [total, setTotal] = useState(0)
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    async function fetchSeries() {
      setLoading(true)
      try {
        const response = await itemsApi.listSeries({ page_size: 100 })
        setSeries(response.items)
        setTotal(response.total)
      } catch (error) {
        console.error('Failed to fetch series:', error)
      } finally {
        setLoading(false)
      }
    }
    fetchSeries()
  }, [])

  const columns: ColumnDef<Item>[] = [
    {
      accessorKey: 'title',
      header: 'Title',
      cell: ({ row }) => (
        <Link
          to={`/series/${row.original.id}`}
          className="font-medium hover:text-primary transition-colors"
        >
          {row.getValue('title')}
        </Link>
      ),
    },
    {
      id: 'subtitles',
      header: 'Subtitles',
      cell: ({ row }) => {
        const languages = row.original.subtitle_languages
        if (languages.length === 0) {
          return <span className="text-muted-foreground">None</span>
        }
        return (
          <span className="text-muted-foreground">
            {languages.join(', ')}
          </span>
        )
      },
    },
    {
      id: 'status',
      header: 'Status',
      cell: ({ row }) => {
        const languages = row.original.subtitle_languages
        if (languages.length > 0) {
          return (
            <Badge variant="success" className="gap-1">
              <Check className="h-3 w-3" />
              Has subtitles
            </Badge>
          )
        }
        return (
          <Badge variant="warning" className="gap-1">
            <AlertTriangle className="h-3 w-3" />
            No subtitles
          </Badge>
        )
      },
    },
    {
      id: 'actions',
      cell: ({ row }) => (
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button variant="ghost" size="icon" className="h-8 w-8">
              <MoreHorizontal className="h-4 w-4" />
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end">
            <DropdownMenuItem asChild>
              <Link to={`/series/${row.original.id}`}>View Episodes</Link>
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
      <div>
        <h1 className="text-2xl font-bold">Series</h1>
        <p className="text-muted-foreground mt-1">{total} series in your library</p>
      </div>

      {/* Table */}
      <DataTable
        columns={columns}
        data={series}
        searchKey="title"
        searchPlaceholder="Search series..."
      />
    </div>
  )
}
