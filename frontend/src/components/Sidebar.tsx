import { Link, useLocation } from 'react-router-dom'
import { useState, useEffect } from 'react'
import { Film, Tv, RefreshCw, Library } from 'lucide-react'
import { librariesApi, type Library as LibraryType } from '@/api'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { Sheet, SheetContent, SheetHeader, SheetTitle } from '@/components/ui/sheet'
import { cn } from '@/lib/utils'

interface SidebarProps {
  isOpen: boolean
  onClose: () => void
}

function SidebarContent() {
  const location = useLocation()
  const [libraries, setLibraries] = useState<LibraryType[]>([])
  const [syncing, setSyncing] = useState(false)

  useEffect(() => {
    librariesApi
      .list()
      .then((response) => {
        setLibraries(response.libraries)
      })
      .catch(console.error)
  }, [])

  const handleSync = async () => {
    setSyncing(true)
    try {
      await librariesApi.sync()
      const response = await librariesApi.list()
      setLibraries(response.libraries)
    } catch (error) {
      console.error('Sync failed:', error)
    } finally {
      setSyncing(false)
    }
  }

  return (
    <div className="flex h-full flex-col">
      {/* Header */}
      <div className="flex items-center gap-2 px-4 py-3 border-b">
        <Library className="h-4 w-4 text-muted-foreground" />
        <span className="text-sm font-medium">Libraries</span>
      </div>

      {/* Library List */}
      <nav className="flex-1 overflow-y-auto p-2 space-y-1">
        {libraries.map((library) => (
          <Link
            key={library.id}
            to={`/library/${library.id}`}
            className={cn(
              "flex items-center gap-3 rounded-md px-3 py-2 text-sm transition-colors",
              location.pathname === `/library/${library.id}`
                ? "bg-primary/10 text-primary"
                : "text-muted-foreground hover:bg-accent hover:text-foreground"
            )}
          >
            {library.type === 'movies' ? (
              <Film className="h-4 w-4" />
            ) : (
              <Tv className="h-4 w-4" />
            )}
            <span className="flex-1 truncate">{library.name}</span>
            <Badge variant="secondary" className="ml-auto">
              {library.item_count}
            </Badge>
          </Link>
        ))}

        {libraries.length === 0 && (
          <p className="text-muted-foreground text-sm text-center py-4">
            No libraries synced yet
          </p>
        )}
      </nav>

      {/* Sync Button */}
      <div className="p-3 border-t">
        <Button
          onClick={handleSync}
          disabled={syncing}
          className="w-full"
          variant="secondary"
        >
          <RefreshCw className={cn("h-4 w-4 mr-2", syncing && "animate-spin")} />
          {syncing ? 'Syncing...' : 'Sync Libraries'}
        </Button>
      </div>
    </div>
  )
}

export function Sidebar({ isOpen, onClose }: SidebarProps) {
  return (
    <>
      {/* Desktop Sidebar */}
      <aside className="hidden lg:flex w-64 flex-col border-r bg-background">
        <SidebarContent />
      </aside>

      {/* Mobile Sidebar (Sheet) */}
      <Sheet open={isOpen} onOpenChange={onClose}>
        <SheetContent side="left" className="w-64 p-0">
          <SheetHeader className="sr-only">
            <SheetTitle>Libraries</SheetTitle>
          </SheetHeader>
          <SidebarContent />
        </SheetContent>
      </Sheet>
    </>
  )
}
