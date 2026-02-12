import { Link, useLocation } from 'react-router-dom'
import { LayoutDashboard, Film, Tv, ListTodo, Settings, Subtitles } from 'lucide-react'
import { cn } from '@/lib/utils'

interface NavItem {
  path: string
  label: string
  icon: React.ReactNode
}

const navItems: NavItem[] = [
  { path: '/', label: 'Dashboard', icon: <LayoutDashboard className="h-4 w-4" /> },
  { path: '/movies', label: 'Movies', icon: <Film className="h-4 w-4" /> },
  { path: '/series', label: 'Series', icon: <Tv className="h-4 w-4" /> },
  { path: '/queue', label: 'Queue', icon: <ListTodo className="h-4 w-4" /> },
]

export function Header() {
  const location = useLocation()

  return (
    <header className="sticky top-0 z-40 w-full border-b bg-background/95 backdrop-blur supports-[backdrop-filter]:bg-background/60">
      <div className="flex h-14 items-center px-4">
        {/* Logo */}
        <Link to="/" className="flex items-center gap-2 mr-6">
          <Subtitles className="h-6 w-6 text-primary" />
          <span className="text-lg font-bold">Submate</span>
        </Link>

        {/* Navigation */}
        <nav className="flex items-center gap-1 flex-1">
          {navItems.map((item) => {
            const isActive = location.pathname === item.path
            return (
              <Link
                key={item.path}
                to={item.path}
                className={cn(
                  "flex items-center gap-2 px-3 py-2 text-sm font-medium rounded-md transition-colors",
                  isActive
                    ? "bg-primary/10 text-primary"
                    : "text-muted-foreground hover:bg-accent hover:text-foreground"
                )}
              >
                {item.icon}
                {item.label}
              </Link>
            )
          })}
        </nav>

        {/* Settings */}
        <Link
          to="/settings"
          className={cn(
            "flex items-center gap-2 px-3 py-2 text-sm font-medium rounded-md transition-colors",
            location.pathname === '/settings'
              ? "bg-primary/10 text-primary"
              : "text-muted-foreground hover:bg-accent hover:text-foreground"
          )}
        >
          <Settings className="h-4 w-4" />
        </Link>
      </div>
    </header>
  )
}
