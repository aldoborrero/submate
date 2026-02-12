# Submate UI Redesign - shadcn/ui + *arr Family Aesthetic

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Redesign Submate UI with shadcn/ui to match *arr family aesthetic (Sonarr/Radarr/Bazarr).

**Architecture:** Replace emoji icons with Lucide, convert poster grids to sortable tables, adopt shadcn/ui component library while keeping Tailwind CSS. Use teal (#20c997) as brand color.

**Tech Stack:** React 19, TypeScript, Vite, Tailwind CSS, shadcn/ui, Lucide icons, @tanstack/react-table

---

## Design Decisions

### Brand Color
- **Primary:** Teal/Cyan `#20c997` (HSL: 162 75% 46%)
- **Rationale:** Distinctive from Bazarr purple, fresh/modern, works with dark theme

### Component Library
- **shadcn/ui:** Copy-paste components built on Radix UI
- **Keeps Tailwind:** No migration needed, just add components
- **Lucide icons:** Default for shadcn, consistent icon set

### Display Mode
- **Table view:** Standard *arr pattern for library management
- **Sortable columns:** Click headers to sort
- **Bulk selection:** Checkboxes for batch operations

### Layout
- **Header:** Fixed h-14, horizontal nav tabs
- **Sidebar:** w-64, library list with counts
- **Responsive:** Sidebar becomes Sheet on mobile

---

## Theme Configuration

### CSS Variables (globals.css)

```css
@layer base {
  :root {
    --background: 224 71% 4%;
    --foreground: 213 31% 91%;
    --card: 224 71% 4%;
    --card-foreground: 213 31% 91%;
    --popover: 224 71% 4%;
    --popover-foreground: 213 31% 91%;
    --primary: 162 75% 46%;
    --primary-foreground: 210 40% 98%;
    --secondary: 222.2 47.4% 11.2%;
    --secondary-foreground: 210 40% 98%;
    --muted: 223 47% 11%;
    --muted-foreground: 215 20% 65%;
    --accent: 216 34% 17%;
    --accent-foreground: 210 40% 98%;
    --destructive: 0 84% 60%;
    --destructive-foreground: 210 40% 98%;
    --border: 216 34% 17%;
    --input: 216 34% 17%;
    --ring: 162 75% 46%;
    --radius: 0.5rem;
  }
}
```

---

## Icon Mapping

| Current Emoji | Lucide Icon | Usage |
|---------------|-------------|-------|
| 📊 | `LayoutDashboard` | Dashboard nav |
| 🎬 | `Film` | Movies nav/library |
| 📺 | `Tv` | Series nav/library |
| 📋 | `ListTodo` | Queue nav |
| ⚙️ | `Settings` | Settings nav |
| 🔄 | `RefreshCw` | Sync button |
| 🎯 | `Subtitles` | Logo icon |
| ☰ | `Menu` | Mobile menu |
| ✓ | `Check` | Success status |
| ⚠ | `AlertTriangle` | Warning status |
| ✕ | `X` | Error/close |

---

## Component Specifications

### shadcn/ui Components Required

```
Button, Input, Table, Badge, DropdownMenu, Sheet, Tabs,
Card, Checkbox, Select, Skeleton, Sonner (toast)
```

### Layout Structure

```
┌─────────────────────────────────────────────────────────┐
│ Header (h-14)                                           │
│ [Menu] Logo    Dashboard | Movies | Series | Queue | ⚙  │
├──────────┬──────────────────────────────────────────────┤
│ Sidebar  │ Main Content                                 │
│ (w-64)   │                                              │
│          │ [Page Title]              [Actions]          │
│ Libraries│ ────────────────────────────────────         │
│ ─────────│ [Search...] [Filter ▼]                       │
│ Movies(n)│ ────────────────────────────────────         │
│ Series(n)│ Table with data                              │
│          │                                              │
│ [Sync]   │ [< 1 2 3 ... 10 >]                           │
└──────────┴──────────────────────────────────────────────┘
```

---

## Table Specifications

### Movies Table

| Column | Width | Sortable | Description |
|--------|-------|----------|-------------|
| Checkbox | 40px | No | Row selection |
| Title | flex | Yes | Movie name, links to detail |
| Year | 80px | Yes | Release year |
| Audio | 100px | Yes | Primary audio language |
| Subtitles | 150px | No | Language badges |
| Status | 100px | Yes | Ready/Missing/Processing |
| Actions | 60px | No | Dropdown menu |

### Series Table

| Column | Width | Sortable | Description |
|--------|-------|----------|-------------|
| Checkbox | 40px | No | Row selection |
| Title | flex | Yes | Series name, links to detail |
| Seasons | 80px | Yes | Season count |
| Episodes | 80px | Yes | Total episodes |
| Subtitles | 120px | No | Coverage (e.g., "58/62") |
| Status | 100px | Yes | Ready/Partial/Missing |
| Actions | 60px | No | Dropdown menu |

### Queue Table

| Column | Width | Sortable | Description |
|--------|-------|----------|-------------|
| Status | 100px | Yes | Running/Pending/Done/Failed |
| Item | flex | Yes | Movie/episode title |
| Type | 80px | Yes | Movie/Episode badge |
| Progress | 100px | No | Progress bar or percentage |
| Started | 100px | Yes | Relative time |
| Actions | 60px | No | Cancel button |

---

## Status Indicators

### Badge Variants

| Status | Color | Icon |
|--------|-------|------|
| Ready | Green (`--primary`) | Check |
| Missing | Yellow (`amber-500`) | AlertTriangle |
| Processing | Blue (`blue-500`) | Loader (spinning) |
| Failed | Red (`destructive`) | X |
| Pending | Gray (`muted`) | Clock |

---

## File Changes Summary

### New Files (shadcn/ui setup)
- `components.json` - shadcn config
- `src/components/ui/*.tsx` - shadcn components
- `src/lib/utils.ts` - cn() helper

### Modified Files
- `src/index.css` - Theme variables
- `src/components/Layout.tsx` - New layout structure
- `src/components/Header.tsx` - Tabs navigation, Lucide icons
- `src/components/Sidebar.tsx` - Sheet for mobile, badges
- `src/pages/Movies.tsx` - Table view
- `src/pages/Series.tsx` - Table view
- `src/pages/Queue.tsx` - Table view
- `src/pages/Dashboard.tsx` - Card components
- `src/pages/Settings.tsx` - Tabs component

### Removed Files
- `src/components/MovieCard.tsx` - Replaced by table row
- `src/components/SeriesCard.tsx` - Replaced by table row
- `src/components/StatsCard.tsx` - Replaced by shadcn Card

---

## Implementation Order

1. **Setup:** Install dependencies, configure shadcn/ui
2. **Theme:** Configure CSS variables and dark theme
3. **Components:** Add required shadcn components
4. **Layout:** Rebuild Header, Sidebar, Layout
5. **Tables:** Convert Movies, Series, Queue to table views
6. **Dashboard:** Update with Card components
7. **Settings:** Update with Tabs component
8. **Polish:** Loading states, animations, responsive tweaks
