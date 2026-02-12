import { useState, useEffect, useCallback } from 'react'
import { ColumnDef } from '@tanstack/react-table'
import { Clock, Loader2, Check, X, MoreHorizontal, RotateCcw, Ban } from 'lucide-react'
import { jobsApi, subscribeToEvents } from '@/api'
import type { Job, JobStatus, SSEEvent } from '@/api'
import { Tabs, TabsList, TabsTrigger, TabsContent } from '@/components/ui/tabs'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { DataTable } from '@/components/DataTable'

type TabFilter = 'all' | JobStatus

interface JobCounts {
  pending: number
  running: number
  completed: number
  failed: number
}

function formatRelativeTime(dateString: string): string {
  const date = new Date(dateString)
  const now = new Date()
  const diff = now.getTime() - date.getTime()
  const minutes = Math.floor(diff / 60000)
  if (minutes < 1) return 'Just now'
  if (minutes < 60) return `${minutes}m ago`
  const hours = Math.floor(minutes / 60)
  if (hours < 24) return `${hours}h ago`
  const days = Math.floor(hours / 24)
  return `${days}d ago`
}

const statusConfig: Record<JobStatus, {
  icon: React.ComponentType<{ className?: string }>
  variant: 'secondary' | 'default' | 'success' | 'destructive'
  label: string
}> = {
  pending: { icon: Clock, variant: 'secondary', label: 'Pending' },
  running: { icon: Loader2, variant: 'default', label: 'Running' },
  completed: { icon: Check, variant: 'success', label: 'Completed' },
  failed: { icon: X, variant: 'destructive', label: 'Failed' },
}

export function QueuePage() {
  const [jobs, setJobs] = useState<Job[]>([])
  const [counts, setCounts] = useState<JobCounts>({
    pending: 0,
    running: 0,
    completed: 0,
    failed: 0,
  })
  const [loading, setLoading] = useState(true)
  const [activeTab, setActiveTab] = useState<TabFilter>('all')

  const fetchJobs = useCallback(async () => {
    try {
      const response = await jobsApi.list({
        status: activeTab === 'all' ? undefined : activeTab,
        page_size: 100,
      })
      setJobs(response.jobs)

      // Calculate counts from the fetched jobs when showing all
      // or fetch all to get accurate counts
      if (activeTab === 'all') {
        const newCounts: JobCounts = {
          pending: 0,
          running: 0,
          completed: 0,
          failed: 0,
        }
        response.jobs.forEach((job) => {
          newCounts[job.status]++
        })
        setCounts(newCounts)
      }
    } catch (error) {
      console.error('Failed to fetch jobs:', error)
    } finally {
      setLoading(false)
    }
  }, [activeTab])

  // Fetch all jobs to get accurate counts
  const fetchCounts = useCallback(async () => {
    try {
      const response = await jobsApi.list({ page_size: 1000 })
      const newCounts: JobCounts = {
        pending: 0,
        running: 0,
        completed: 0,
        failed: 0,
      }
      response.jobs.forEach((job) => {
        newCounts[job.status]++
      })
      setCounts(newCounts)
    } catch (error) {
      console.error('Failed to fetch job counts:', error)
    }
  }, [])

  // Fetch jobs when tab changes
  useEffect(() => {
    setLoading(true)
    fetchJobs()
  }, [fetchJobs])

  // Fetch counts on initial load and when jobs change
  useEffect(() => {
    fetchCounts()
  }, [fetchCounts])

  // Subscribe to SSE events for real-time updates
  useEffect(() => {
    const unsubscribe = subscribeToEvents((event: SSEEvent) => {
      if (event.event_type.startsWith('job.')) {
        fetchJobs()
        fetchCounts()
      }
    })
    return unsubscribe
  }, [fetchJobs, fetchCounts])

  const handleRetry = async (jobId: string) => {
    try {
      await jobsApi.retry(jobId)
      fetchJobs()
      fetchCounts()
    } catch (error) {
      console.error('Failed to retry job:', error)
    }
  }

  const handleCancel = async (jobId: string) => {
    try {
      await jobsApi.cancel(jobId)
      fetchJobs()
      fetchCounts()
    } catch (error) {
      console.error('Failed to cancel job:', error)
    }
  }

  const columns: ColumnDef<Job>[] = [
    {
      accessorKey: 'status',
      header: 'Status',
      size: 100,
      cell: ({ row }) => {
        const status = row.getValue('status') as JobStatus
        const config = statusConfig[status]
        const Icon = config.icon
        return (
          <Badge variant={config.variant} className="gap-1">
            <Icon className={`h-3 w-3 ${status === 'running' ? 'animate-spin' : ''}`} />
            {config.label}
          </Badge>
        )
      },
    },
    {
      accessorKey: 'item_title',
      header: 'Item',
      cell: ({ row }) => {
        const job = row.original
        return (
          <div>
            <span className="font-medium">{job.item_title}</span>
            {job.error && (
              <p className="text-xs text-destructive mt-0.5">{job.error}</p>
            )}
          </div>
        )
      },
    },
    {
      accessorKey: 'language',
      header: 'Language',
      size: 80,
      cell: ({ row }) => (
        <Badge variant="outline">{row.getValue('language')}</Badge>
      ),
    },
    {
      id: 'progress',
      header: 'Progress',
      size: 100,
      enableSorting: false,
      cell: ({ row }) => {
        const status = row.original.status
        if (status === 'running') {
          return (
            <div className="flex items-center gap-2">
              <Loader2 className="h-4 w-4 animate-spin text-primary" />
              <span className="text-sm text-muted-foreground">Processing</span>
            </div>
          )
        }
        return <span className="text-muted-foreground">-</span>
      },
    },
    {
      accessorKey: 'created_at',
      header: 'Started',
      size: 100,
      cell: ({ row }) => (
        <span className="text-sm text-muted-foreground">
          {formatRelativeTime(row.getValue('created_at'))}
        </span>
      ),
    },
    {
      id: 'actions',
      header: '',
      size: 60,
      enableSorting: false,
      cell: ({ row }) => {
        const job = row.original
        const canCancel = job.status === 'pending'
        const canRetry = job.status === 'failed'

        if (!canCancel && !canRetry) {
          return null
        }

        return (
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button variant="ghost" size="icon" className="h-8 w-8">
                <MoreHorizontal className="h-4 w-4" />
                <span className="sr-only">Open menu</span>
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end">
              {canCancel && (
                <DropdownMenuItem onClick={() => handleCancel(job.id)}>
                  <Ban className="mr-2 h-4 w-4" />
                  Cancel
                </DropdownMenuItem>
              )}
              {canRetry && (
                <DropdownMenuItem onClick={() => handleRetry(job.id)}>
                  <RotateCcw className="mr-2 h-4 w-4" />
                  Retry
                </DropdownMenuItem>
              )}
            </DropdownMenuContent>
          </DropdownMenu>
        )
      },
    },
  ]

  const totalCount = counts.pending + counts.running + counts.completed + counts.failed

  const renderJobTable = () => {
    if (loading) {
      return (
        <div className="text-center py-12 text-muted-foreground">
          Loading jobs...
        </div>
      )
    }

    return (
      <DataTable
        columns={columns}
        data={jobs}
        searchKey="item_title"
        searchPlaceholder="Search jobs..."
      />
    )
  }

  return (
    <div className="space-y-6">
      {/* Header */}
      <div>
        <h1 className="text-2xl font-bold">Queue</h1>
        <p className="text-muted-foreground mt-1">Manage your transcription jobs</p>
      </div>

      {/* Tabs */}
      <Tabs value={activeTab} onValueChange={(value) => setActiveTab(value as TabFilter)}>
        <TabsList>
          <TabsTrigger value="all">
            All
            <Badge variant="secondary" className="ml-2">
              {totalCount}
            </Badge>
          </TabsTrigger>
          <TabsTrigger value="pending">
            Pending
            <Badge variant="secondary" className="ml-2">
              {counts.pending}
            </Badge>
          </TabsTrigger>
          <TabsTrigger value="running">
            Running
            <Badge variant="secondary" className="ml-2">
              {counts.running}
            </Badge>
          </TabsTrigger>
          <TabsTrigger value="completed">
            Completed
            <Badge variant="secondary" className="ml-2">
              {counts.completed}
            </Badge>
          </TabsTrigger>
          <TabsTrigger value="failed">
            Failed
            <Badge variant="secondary" className="ml-2">
              {counts.failed}
            </Badge>
          </TabsTrigger>
        </TabsList>

        <TabsContent value="all">{renderJobTable()}</TabsContent>
        <TabsContent value="pending">{renderJobTable()}</TabsContent>
        <TabsContent value="running">{renderJobTable()}</TabsContent>
        <TabsContent value="completed">{renderJobTable()}</TabsContent>
        <TabsContent value="failed">{renderJobTable()}</TabsContent>
      </Tabs>
    </div>
  )
}
