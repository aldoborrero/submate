import { Routes, Route } from 'react-router-dom'
import { Layout } from '@/components'
import {
  DashboardPage,
  MoviesPage,
  SeriesPage,
  SeriesDetailPage,
  ItemDetailPage,
  QueuePage,
  SettingsPage,
} from '@/pages'

function App() {
  return (
    <Layout>
      <Routes>
        <Route path="/" element={<DashboardPage />} />
        <Route path="/movies" element={<MoviesPage />} />
        <Route path="/series" element={<SeriesPage />} />
        <Route path="/series/:id" element={<SeriesDetailPage />} />
        <Route path="/item/:id" element={<ItemDetailPage />} />
        <Route path="/queue" element={<QueuePage />} />
        <Route path="/settings" element={<SettingsPage />} />
        <Route path="/library/:id" element={<LibraryPlaceholder />} />
      </Routes>
    </Layout>
  )
}

function LibraryPlaceholder() {
  return <div className="text-white">Library Detail - Coming Soon</div>
}

export default App
