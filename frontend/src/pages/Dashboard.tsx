import { useState, useEffect } from 'react'
import { Library, Film, Clock, XCircle } from 'lucide-react'
import { librariesApi, jobsApi, subscribeToEvents } from '@/api'
import type { Library as LibraryType, Job, SSEEvent } from '@/api'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { Skeleton } from '@/components/ui/skeleton'
import { cn } from '@/lib/utils'

export function DashboardPage() {
  const [libraries, setLibraries] = useState<LibraryType[]>([])
  const [jobs, setJobs] = useState<Job[]>([])
  const [jobCounts, setJobCounts] = useState({
    pending: 0,
    running: 0,
    completed: 0,
    failed: 0,
  })
  const [loading, setLoading] = useState(true)

  const calculateJobCounts = (jobsList: Job[]) => {
    const counts = { pending: 0, running: 0, completed: 0, failed: 0 }
    jobsList.forEach((job) => {
      counts[job.status]++
    })
    return counts
  }

  useEffect(() => {
    async function fetchData() {
      try {
        const [libResponse, jobResponse] = await Promise.all([
          librariesApi.list(),
          jobsApi.list({ page_size: 10 }),
        ])
        setLibraries(libResponse.libraries)
        setJobs(jobResponse.jobs)
        setJobCounts(calculateJobCounts(jobResponse.jobs))
      } catch (error) {
        console.error('Failed to fetch dashboard data:', error)
      } finally {
        setLoading(false)
      }
    }
    fetchData()
  }, [])

  useEffect(() => {
    const unsubscribe = subscribeToEvents((event: SSEEvent) => {
      if (event.event_type.startsWith('job.')) {
        jobsApi.list({ page_size: 10 }).then((response) => {
          setJobs(response.jobs)
          setJobCounts(calculateJobCounts(response.jobs))
        })
      } else if (event.event_type === 'sync.completed') {
        librariesApi.list().then((response) => {
          setLibraries(response.libraries)
        })
      }
    })
    return unsubscribe
  }, [])

  const handleRetry = async (jobId: string) => {
    try {
      await jobsApi.retry(jobId)
      const response = await jobsApi.list({ page_size: 10 })
      setJobs(response.jobs)
      setJobCounts(calculateJobCounts(response.jobs))
    } catch (error) {
      console.error('Retry failed:', error)
    }
  }

  const totalItems = libraries.reduce((sum, lib) => sum + lib.item_count, 0)

  if (loading) {
    return (
      <div className="space-y-8">
        <div>
          <Skeleton className="h-8 w-32" />
          <Skeleton className="h-4 w-48 mt-2" />
        </div>
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
          {[...Array(4)].map((_, i) => (
            <Skeleton key={i} className="h-32" />
          ))}
        </div>
      </div>
    )
  }

  const stats = [
    { title: 'Libraries', value: libraries.length, icon: Library, color: 'text-blue-500' },
    { title: 'Total Items', value: totalItems, icon: Film, color: 'text-emerald-500' },
    { title: 'Pending Jobs', value: jobCounts.pending + jobCounts.running, icon: Clock, color: 'text-amber-500' },
    { title: 'Failed Jobs', value: jobCounts.failed, icon: XCircle, color: 'text-red-500' },
  ]

  return (
    <div className="space-y-8">
      {/* Page Header */}
      <div>
        <h1 className="text-2xl font-bold">Dashboard</h1>
        <p className="text-muted-foreground mt-1">Overview of your Submate instance</p>
      </div>

      {/* Stats Cards */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
        {stats.map((stat) => (
          <Card key={stat.title}>
            <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
              <CardTitle className="text-sm font-medium">{stat.title}</CardTitle>
              <stat.icon className={cn("h-4 w-4", stat.color)} />
            </CardHeader>
            <CardContent>
              <div className="text-2xl font-bold">{stat.value}</div>
            </CardContent>
          </Card>
        ))}
      </div>

      {/* Recent Activity */}
      <div>
        <h2 className="text-xl font-semibold mb-4">Recent Jobs</h2>
        <div className="space-y-3">
          {jobs.length > 0 ? (
            jobs.map((job) => (
              <Card key={job.id}>
                <CardContent className="flex items-center justify-between p-4">
                  <div className="flex items-center gap-4">
                    <Badge
                      variant={
                        job.status === 'completed' ? 'success' :
                        job.status === 'failed' ? 'destructive' :
                        job.status === 'running' ? 'default' : 'secondary'
                      }
                    >
                      {job.status}
                    </Badge>
                    <div>
                      <p className="font-medium">{job.item_title}</p>
                      <p className="text-sm text-muted-foreground">{job.language}</p>
                    </div>
                  </div>
                  {job.status === 'failed' && (
                    <Button variant="outline" size="sm" onClick={() => handleRetry(job.id)}>
                      Retry
                    </Button>
                  )}
                </CardContent>
              </Card>
            ))
          ) : (
            <Card>
              <CardContent className="text-center py-8 text-muted-foreground">
                No jobs yet. Click on a movie or series to start transcription.
              </CardContent>
            </Card>
          )}
        </div>
      </div>
    </div>
  )
}
