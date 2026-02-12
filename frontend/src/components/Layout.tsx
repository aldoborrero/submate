import { useState } from 'react'
import { Menu } from 'lucide-react'
import { Header } from './Header'
import { Sidebar } from './Sidebar'
import { Button } from '@/components/ui/button'

interface LayoutProps {
  children: React.ReactNode
}

export function Layout({ children }: LayoutProps) {
  const [sidebarOpen, setSidebarOpen] = useState(false)

  return (
    <div className="flex flex-col h-screen bg-background">
      <Header />

      <div className="flex flex-1 overflow-hidden">
        {/* Mobile menu button */}
        <Button
          variant="outline"
          size="icon"
          onClick={() => setSidebarOpen(true)}
          className="lg:hidden fixed bottom-4 left-4 z-30 h-12 w-12 rounded-full shadow-lg"
        >
          <Menu className="h-5 w-5" />
        </Button>

        <Sidebar isOpen={sidebarOpen} onClose={() => setSidebarOpen(false)} />

        {/* Main Content */}
        <main className="flex-1 overflow-y-auto p-6 lg:p-8">
          <div className="max-w-7xl mx-auto">{children}</div>
        </main>
      </div>
    </div>
  )
}
