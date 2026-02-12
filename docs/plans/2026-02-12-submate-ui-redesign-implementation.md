# Submate UI Redesign Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Migrate Submate UI from custom Tailwind components to shadcn/ui with teal brand color and *arr family table-based aesthetics.

**Architecture:** Install shadcn/ui with dark theme, replace emoji icons with Lucide, convert grid-based displays to sortable tables using @tanstack/react-table, update color scheme to teal primary.

**Tech Stack:** React 19, TypeScript, Vite, Tailwind CSS, shadcn/ui, Lucide React, @tanstack/react-table

---

## Task 1: Install shadcn/ui Dependencies

**Files:**
- Modify: `frontend/package.json`
- Create: `frontend/components.json`
- Modify: `frontend/tailwind.config.js`
- Modify: `frontend/tsconfig.json`

**Step 1: Install core dependencies**

Run:
```bash
cd /home/aldo/Dev/aldoborrero/submate/.worktrees/submate-ui/frontend && bun add class-variance-authority clsx tailwind-merge lucide-react @radix-ui/react-slot
```

**Step 2: Install tailwindcss-animate**

Run:
```bash
cd /home/aldo/Dev/aldoborrero/submate/.worktrees/submate-ui/frontend && bun add -D tailwindcss-animate
```

**Step 3: Create components.json**

Create `frontend/components.json`:
```json
{
  "$schema": "https://ui.shadcn.com/schema.json",
  "style": "default",
  "rsc": false,
  "tsx": true,
  "tailwind": {
    "config": "tailwind.config.js",
    "css": "src/index.css",
    "baseColor": "slate",
    "cssVariables": true,
    "prefix": ""
  },
  "aliases": {
    "components": "@/components",
    "utils": "@/lib/utils",
    "ui": "@/components/ui",
    "lib": "@/lib",
    "hooks": "@/hooks"
  }
}
```

**Step 4: Update tailwind.config.js**

Replace `frontend/tailwind.config.js`:
```javascript
/** @type {import('tailwindcss').Config} */
export default {
  darkMode: ["class"],
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {
      colors: {
        border: "hsl(var(--border))",
        input: "hsl(var(--input))",
        ring: "hsl(var(--ring))",
        background: "hsl(var(--background))",
        foreground: "hsl(var(--foreground))",
        primary: {
          DEFAULT: "hsl(var(--primary))",
          foreground: "hsl(var(--primary-foreground))",
        },
        secondary: {
          DEFAULT: "hsl(var(--secondary))",
          foreground: "hsl(var(--secondary-foreground))",
        },
        destructive: {
          DEFAULT: "hsl(var(--destructive))",
          foreground: "hsl(var(--destructive-foreground))",
        },
        muted: {
          DEFAULT: "hsl(var(--muted))",
          foreground: "hsl(var(--muted-foreground))",
        },
        accent: {
          DEFAULT: "hsl(var(--accent))",
          foreground: "hsl(var(--accent-foreground))",
        },
        popover: {
          DEFAULT: "hsl(var(--popover))",
          foreground: "hsl(var(--popover-foreground))",
        },
        card: {
          DEFAULT: "hsl(var(--card))",
          foreground: "hsl(var(--card-foreground))",
        },
      },
      borderRadius: {
        lg: "var(--radius)",
        md: "calc(var(--radius) - 2px)",
        sm: "calc(var(--radius) - 4px)",
      },
    },
  },
  plugins: [require("tailwindcss-animate")],
}
```

**Step 5: Commit**

```bash
git add frontend/package.json frontend/components.json frontend/tailwind.config.js
git commit -m "feat(ui): install shadcn/ui dependencies and configure tailwind"
```

---

## Task 2: Set Up Theme and Utils

**Files:**
- Modify: `frontend/src/index.css`
- Create: `frontend/src/lib/utils.ts`

**Step 1: Update index.css with CSS variables**

Replace `frontend/src/index.css`:
```css
@tailwind base;
@tailwind components;
@tailwind utilities;

@layer base {
  :root {
    --background: 222.2 84% 4.9%;
    --foreground: 210 40% 98%;
    --card: 222.2 84% 4.9%;
    --card-foreground: 210 40% 98%;
    --popover: 222.2 84% 4.9%;
    --popover-foreground: 210 40% 98%;
    --primary: 162 75% 46%;
    --primary-foreground: 210 40% 98%;
    --secondary: 217.2 32.6% 17.5%;
    --secondary-foreground: 210 40% 98%;
    --muted: 217.2 32.6% 17.5%;
    --muted-foreground: 215 20.2% 65.1%;
    --accent: 217.2 32.6% 17.5%;
    --accent-foreground: 210 40% 98%;
    --destructive: 0 62.8% 30.6%;
    --destructive-foreground: 210 40% 98%;
    --border: 217.2 32.6% 17.5%;
    --input: 217.2 32.6% 17.5%;
    --ring: 162 75% 46%;
    --radius: 0.5rem;
  }
}

@layer base {
  * {
    @apply border-border;
  }
  body {
    @apply bg-background text-foreground min-h-screen;
    font-family: Inter, system-ui, Avenir, Helvetica, Arial, sans-serif;
  }
}
```

**Step 2: Create utils.ts**

Create `frontend/src/lib/utils.ts`:
```typescript
import { type ClassValue, clsx } from "clsx"
import { twMerge } from "tailwind-merge"

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}
```

**Step 3: Commit**

```bash
git add frontend/src/index.css frontend/src/lib/utils.ts
git commit -m "feat(ui): add teal theme CSS variables and cn utility"
```

---

## Task 3: Add Core shadcn/ui Components

**Files:**
- Create: `frontend/src/components/ui/button.tsx`
- Create: `frontend/src/components/ui/input.tsx`
- Create: `frontend/src/components/ui/badge.tsx`
- Create: `frontend/src/components/ui/card.tsx`
- Create: `frontend/src/components/ui/skeleton.tsx`

**Step 1: Create Button component**

Create `frontend/src/components/ui/button.tsx`:
```typescript
import * as React from "react"
import { Slot } from "@radix-ui/react-slot"
import { cva, type VariantProps } from "class-variance-authority"
import { cn } from "@/lib/utils"

const buttonVariants = cva(
  "inline-flex items-center justify-center whitespace-nowrap rounded-md text-sm font-medium ring-offset-background transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:pointer-events-none disabled:opacity-50",
  {
    variants: {
      variant: {
        default: "bg-primary text-primary-foreground hover:bg-primary/90",
        destructive: "bg-destructive text-destructive-foreground hover:bg-destructive/90",
        outline: "border border-input bg-background hover:bg-accent hover:text-accent-foreground",
        secondary: "bg-secondary text-secondary-foreground hover:bg-secondary/80",
        ghost: "hover:bg-accent hover:text-accent-foreground",
        link: "text-primary underline-offset-4 hover:underline",
      },
      size: {
        default: "h-10 px-4 py-2",
        sm: "h-9 rounded-md px-3",
        lg: "h-11 rounded-md px-8",
        icon: "h-10 w-10",
      },
    },
    defaultVariants: {
      variant: "default",
      size: "default",
    },
  }
)

export interface ButtonProps
  extends React.ButtonHTMLAttributes<HTMLButtonElement>,
    VariantProps<typeof buttonVariants> {
  asChild?: boolean
}

const Button = React.forwardRef<HTMLButtonElement, ButtonProps>(
  ({ className, variant, size, asChild = false, ...props }, ref) => {
    const Comp = asChild ? Slot : "button"
    return (
      <Comp
        className={cn(buttonVariants({ variant, size, className }))}
        ref={ref}
        {...props}
      />
    )
  }
)
Button.displayName = "Button"

export { Button, buttonVariants }
```

**Step 2: Create Input component**

Create `frontend/src/components/ui/input.tsx`:
```typescript
import * as React from "react"
import { cn } from "@/lib/utils"

export interface InputProps extends React.InputHTMLAttributes<HTMLInputElement> {}

const Input = React.forwardRef<HTMLInputElement, InputProps>(
  ({ className, type, ...props }, ref) => {
    return (
      <input
        type={type}
        className={cn(
          "flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background file:border-0 file:bg-transparent file:text-sm file:font-medium placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50",
          className
        )}
        ref={ref}
        {...props}
      />
    )
  }
)
Input.displayName = "Input"

export { Input }
```

**Step 3: Create Badge component**

Create `frontend/src/components/ui/badge.tsx`:
```typescript
import * as React from "react"
import { cva, type VariantProps } from "class-variance-authority"
import { cn } from "@/lib/utils"

const badgeVariants = cva(
  "inline-flex items-center rounded-full border px-2.5 py-0.5 text-xs font-semibold transition-colors focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2",
  {
    variants: {
      variant: {
        default: "border-transparent bg-primary text-primary-foreground hover:bg-primary/80",
        secondary: "border-transparent bg-secondary text-secondary-foreground hover:bg-secondary/80",
        destructive: "border-transparent bg-destructive text-destructive-foreground hover:bg-destructive/80",
        outline: "text-foreground",
        success: "border-transparent bg-emerald-500/20 text-emerald-400",
        warning: "border-transparent bg-amber-500/20 text-amber-400",
      },
    },
    defaultVariants: {
      variant: "default",
    },
  }
)

export interface BadgeProps
  extends React.HTMLAttributes<HTMLDivElement>,
    VariantProps<typeof badgeVariants> {}

function Badge({ className, variant, ...props }: BadgeProps) {
  return <div className={cn(badgeVariants({ variant }), className)} {...props} />
}

export { Badge, badgeVariants }
```

**Step 4: Create Card component**

Create `frontend/src/components/ui/card.tsx`:
```typescript
import * as React from "react"
import { cn } from "@/lib/utils"

const Card = React.forwardRef<HTMLDivElement, React.HTMLAttributes<HTMLDivElement>>(
  ({ className, ...props }, ref) => (
    <div
      ref={ref}
      className={cn("rounded-lg border bg-card text-card-foreground shadow-sm", className)}
      {...props}
    />
  )
)
Card.displayName = "Card"

const CardHeader = React.forwardRef<HTMLDivElement, React.HTMLAttributes<HTMLDivElement>>(
  ({ className, ...props }, ref) => (
    <div ref={ref} className={cn("flex flex-col space-y-1.5 p-6", className)} {...props} />
  )
)
CardHeader.displayName = "CardHeader"

const CardTitle = React.forwardRef<HTMLParagraphElement, React.HTMLAttributes<HTMLHeadingElement>>(
  ({ className, ...props }, ref) => (
    <h3 ref={ref} className={cn("text-2xl font-semibold leading-none tracking-tight", className)} {...props} />
  )
)
CardTitle.displayName = "CardTitle"

const CardDescription = React.forwardRef<HTMLParagraphElement, React.HTMLAttributes<HTMLParagraphElement>>(
  ({ className, ...props }, ref) => (
    <p ref={ref} className={cn("text-sm text-muted-foreground", className)} {...props} />
  )
)
CardDescription.displayName = "CardDescription"

const CardContent = React.forwardRef<HTMLDivElement, React.HTMLAttributes<HTMLDivElement>>(
  ({ className, ...props }, ref) => (
    <div ref={ref} className={cn("p-6 pt-0", className)} {...props} />
  )
)
CardContent.displayName = "CardContent"

const CardFooter = React.forwardRef<HTMLDivElement, React.HTMLAttributes<HTMLDivElement>>(
  ({ className, ...props }, ref) => (
    <div ref={ref} className={cn("flex items-center p-6 pt-0", className)} {...props} />
  )
)
CardFooter.displayName = "CardFooter"

export { Card, CardHeader, CardFooter, CardTitle, CardDescription, CardContent }
```

**Step 5: Create Skeleton component**

Create `frontend/src/components/ui/skeleton.tsx`:
```typescript
import { cn } from "@/lib/utils"

function Skeleton({ className, ...props }: React.HTMLAttributes<HTMLDivElement>) {
  return (
    <div
      className={cn("animate-pulse rounded-md bg-muted", className)}
      {...props}
    />
  )
}

export { Skeleton }
```

**Step 6: Commit**

```bash
git add frontend/src/components/ui/
git commit -m "feat(ui): add core shadcn/ui components (button, input, badge, card, skeleton)"
```

---

## Task 4: Add Table Components

**Files:**
- Create: `frontend/src/components/ui/table.tsx`
- Create: `frontend/src/components/ui/checkbox.tsx`

**Step 1: Install @tanstack/react-table**

Run:
```bash
cd /home/aldo/Dev/aldoborrero/submate/.worktrees/submate-ui/frontend && bun add @tanstack/react-table
```

**Step 2: Create Table component**

Create `frontend/src/components/ui/table.tsx`:
```typescript
import * as React from "react"
import { cn } from "@/lib/utils"

const Table = React.forwardRef<HTMLTableElement, React.HTMLAttributes<HTMLTableElement>>(
  ({ className, ...props }, ref) => (
    <div className="relative w-full overflow-auto">
      <table
        ref={ref}
        className={cn("w-full caption-bottom text-sm", className)}
        {...props}
      />
    </div>
  )
)
Table.displayName = "Table"

const TableHeader = React.forwardRef<HTMLTableSectionElement, React.HTMLAttributes<HTMLTableSectionElement>>(
  ({ className, ...props }, ref) => (
    <thead ref={ref} className={cn("[&_tr]:border-b", className)} {...props} />
  )
)
TableHeader.displayName = "TableHeader"

const TableBody = React.forwardRef<HTMLTableSectionElement, React.HTMLAttributes<HTMLTableSectionElement>>(
  ({ className, ...props }, ref) => (
    <tbody ref={ref} className={cn("[&_tr:last-child]:border-0", className)} {...props} />
  )
)
TableBody.displayName = "TableBody"

const TableFooter = React.forwardRef<HTMLTableSectionElement, React.HTMLAttributes<HTMLTableSectionElement>>(
  ({ className, ...props }, ref) => (
    <tfoot
      ref={ref}
      className={cn("border-t bg-muted/50 font-medium [&>tr]:last:border-b-0", className)}
      {...props}
    />
  )
)
TableFooter.displayName = "TableFooter"

const TableRow = React.forwardRef<HTMLTableRowElement, React.HTMLAttributes<HTMLTableRowElement>>(
  ({ className, ...props }, ref) => (
    <tr
      ref={ref}
      className={cn(
        "border-b transition-colors hover:bg-muted/50 data-[state=selected]:bg-muted",
        className
      )}
      {...props}
    />
  )
)
TableRow.displayName = "TableRow"

const TableHead = React.forwardRef<HTMLTableCellElement, React.ThHTMLAttributes<HTMLTableCellElement>>(
  ({ className, ...props }, ref) => (
    <th
      ref={ref}
      className={cn(
        "h-12 px-4 text-left align-middle font-medium text-muted-foreground [&:has([role=checkbox])]:pr-0",
        className
      )}
      {...props}
    />
  )
)
TableHead.displayName = "TableHead"

const TableCell = React.forwardRef<HTMLTableCellElement, React.TdHTMLAttributes<HTMLTableCellElement>>(
  ({ className, ...props }, ref) => (
    <td
      ref={ref}
      className={cn("p-4 align-middle [&:has([role=checkbox])]:pr-0", className)}
      {...props}
    />
  )
)
TableCell.displayName = "TableCell"

const TableCaption = React.forwardRef<HTMLTableCaptionElement, React.HTMLAttributes<HTMLTableCaptionElement>>(
  ({ className, ...props }, ref) => (
    <caption ref={ref} className={cn("mt-4 text-sm text-muted-foreground", className)} {...props} />
  )
)
TableCaption.displayName = "TableCaption"

export {
  Table,
  TableHeader,
  TableBody,
  TableFooter,
  TableHead,
  TableRow,
  TableCell,
  TableCaption,
}
```

**Step 3: Create Checkbox component**

Create `frontend/src/components/ui/checkbox.tsx`:
```typescript
import * as React from "react"
import { Check } from "lucide-react"
import { cn } from "@/lib/utils"

export interface CheckboxProps extends React.InputHTMLAttributes<HTMLInputElement> {}

const Checkbox = React.forwardRef<HTMLInputElement, CheckboxProps>(
  ({ className, ...props }, ref) => {
    return (
      <div className="relative">
        <input
          type="checkbox"
          ref={ref}
          className={cn(
            "peer h-4 w-4 shrink-0 rounded-sm border border-primary ring-offset-background focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50 appearance-none bg-background checked:bg-primary checked:border-primary",
            className
          )}
          {...props}
        />
        <Check className="absolute top-0 left-0 h-4 w-4 text-primary-foreground pointer-events-none opacity-0 peer-checked:opacity-100" />
      </div>
    )
  }
)
Checkbox.displayName = "Checkbox"

export { Checkbox }
```

**Step 4: Commit**

```bash
git add frontend/src/components/ui/table.tsx frontend/src/components/ui/checkbox.tsx frontend/package.json
git commit -m "feat(ui): add table and checkbox components with @tanstack/react-table"
```

---

## Task 5: Add Navigation Components

**Files:**
- Create: `frontend/src/components/ui/sheet.tsx`
- Create: `frontend/src/components/ui/tabs.tsx`
- Create: `frontend/src/components/ui/dropdown-menu.tsx`

**Step 1: Install Radix UI dependencies**

Run:
```bash
cd /home/aldo/Dev/aldoborrero/submate/.worktrees/submate-ui/frontend && bun add @radix-ui/react-dialog @radix-ui/react-tabs @radix-ui/react-dropdown-menu
```

**Step 2: Create Sheet component**

Create `frontend/src/components/ui/sheet.tsx`:
```typescript
import * as React from "react"
import * as SheetPrimitive from "@radix-ui/react-dialog"
import { cva, type VariantProps } from "class-variance-authority"
import { X } from "lucide-react"
import { cn } from "@/lib/utils"

const Sheet = SheetPrimitive.Root
const SheetTrigger = SheetPrimitive.Trigger
const SheetClose = SheetPrimitive.Close
const SheetPortal = SheetPrimitive.Portal

const SheetOverlay = React.forwardRef<
  React.ElementRef<typeof SheetPrimitive.Overlay>,
  React.ComponentPropsWithoutRef<typeof SheetPrimitive.Overlay>
>(({ className, ...props }, ref) => (
  <SheetPrimitive.Overlay
    className={cn(
      "fixed inset-0 z-50 bg-black/80 data-[state=open]:animate-in data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0",
      className
    )}
    {...props}
    ref={ref}
  />
))
SheetOverlay.displayName = SheetPrimitive.Overlay.displayName

const sheetVariants = cva(
  "fixed z-50 gap-4 bg-background p-6 shadow-lg transition ease-in-out data-[state=open]:animate-in data-[state=closed]:animate-out data-[state=closed]:duration-300 data-[state=open]:duration-500",
  {
    variants: {
      side: {
        top: "inset-x-0 top-0 border-b data-[state=closed]:slide-out-to-top data-[state=open]:slide-in-from-top",
        bottom: "inset-x-0 bottom-0 border-t data-[state=closed]:slide-out-to-bottom data-[state=open]:slide-in-from-bottom",
        left: "inset-y-0 left-0 h-full w-3/4 border-r data-[state=closed]:slide-out-to-left data-[state=open]:slide-in-from-left sm:max-w-sm",
        right: "inset-y-0 right-0 h-full w-3/4 border-l data-[state=closed]:slide-out-to-right data-[state=open]:slide-in-from-right sm:max-w-sm",
      },
    },
    defaultVariants: {
      side: "right",
    },
  }
)

interface SheetContentProps
  extends React.ComponentPropsWithoutRef<typeof SheetPrimitive.Content>,
    VariantProps<typeof sheetVariants> {}

const SheetContent = React.forwardRef<
  React.ElementRef<typeof SheetPrimitive.Content>,
  SheetContentProps
>(({ side = "right", className, children, ...props }, ref) => (
  <SheetPortal>
    <SheetOverlay />
    <SheetPrimitive.Content ref={ref} className={cn(sheetVariants({ side }), className)} {...props}>
      {children}
      <SheetPrimitive.Close className="absolute right-4 top-4 rounded-sm opacity-70 ring-offset-background transition-opacity hover:opacity-100 focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2 disabled:pointer-events-none data-[state=open]:bg-secondary">
        <X className="h-4 w-4" />
        <span className="sr-only">Close</span>
      </SheetPrimitive.Close>
    </SheetPrimitive.Content>
  </SheetPortal>
))
SheetContent.displayName = SheetPrimitive.Content.displayName

const SheetHeader = ({ className, ...props }: React.HTMLAttributes<HTMLDivElement>) => (
  <div className={cn("flex flex-col space-y-2 text-center sm:text-left", className)} {...props} />
)
SheetHeader.displayName = "SheetHeader"

const SheetFooter = ({ className, ...props }: React.HTMLAttributes<HTMLDivElement>) => (
  <div className={cn("flex flex-col-reverse sm:flex-row sm:justify-end sm:space-x-2", className)} {...props} />
)
SheetFooter.displayName = "SheetFooter"

const SheetTitle = React.forwardRef<
  React.ElementRef<typeof SheetPrimitive.Title>,
  React.ComponentPropsWithoutRef<typeof SheetPrimitive.Title>
>(({ className, ...props }, ref) => (
  <SheetPrimitive.Title ref={ref} className={cn("text-lg font-semibold text-foreground", className)} {...props} />
))
SheetTitle.displayName = SheetPrimitive.Title.displayName

const SheetDescription = React.forwardRef<
  React.ElementRef<typeof SheetPrimitive.Description>,
  React.ComponentPropsWithoutRef<typeof SheetPrimitive.Description>
>(({ className, ...props }, ref) => (
  <SheetPrimitive.Description ref={ref} className={cn("text-sm text-muted-foreground", className)} {...props} />
))
SheetDescription.displayName = SheetPrimitive.Description.displayName

export {
  Sheet,
  SheetPortal,
  SheetOverlay,
  SheetTrigger,
  SheetClose,
  SheetContent,
  SheetHeader,
  SheetFooter,
  SheetTitle,
  SheetDescription,
}
```

**Step 3: Create Tabs component**

Create `frontend/src/components/ui/tabs.tsx`:
```typescript
import * as React from "react"
import * as TabsPrimitive from "@radix-ui/react-tabs"
import { cn } from "@/lib/utils"

const Tabs = TabsPrimitive.Root

const TabsList = React.forwardRef<
  React.ElementRef<typeof TabsPrimitive.List>,
  React.ComponentPropsWithoutRef<typeof TabsPrimitive.List>
>(({ className, ...props }, ref) => (
  <TabsPrimitive.List
    ref={ref}
    className={cn(
      "inline-flex h-10 items-center justify-center rounded-md bg-muted p-1 text-muted-foreground",
      className
    )}
    {...props}
  />
))
TabsList.displayName = TabsPrimitive.List.displayName

const TabsTrigger = React.forwardRef<
  React.ElementRef<typeof TabsPrimitive.Trigger>,
  React.ComponentPropsWithoutRef<typeof TabsPrimitive.Trigger>
>(({ className, ...props }, ref) => (
  <TabsPrimitive.Trigger
    ref={ref}
    className={cn(
      "inline-flex items-center justify-center whitespace-nowrap rounded-sm px-3 py-1.5 text-sm font-medium ring-offset-background transition-all focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:pointer-events-none disabled:opacity-50 data-[state=active]:bg-background data-[state=active]:text-foreground data-[state=active]:shadow-sm",
      className
    )}
    {...props}
  />
))
TabsTrigger.displayName = TabsPrimitive.Trigger.displayName

const TabsContent = React.forwardRef<
  React.ElementRef<typeof TabsPrimitive.Content>,
  React.ComponentPropsWithoutRef<typeof TabsPrimitive.Content>
>(({ className, ...props }, ref) => (
  <TabsPrimitive.Content
    ref={ref}
    className={cn(
      "mt-2 ring-offset-background focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2",
      className
    )}
    {...props}
  />
))
TabsContent.displayName = TabsPrimitive.Content.displayName

export { Tabs, TabsList, TabsTrigger, TabsContent }
```

**Step 4: Create DropdownMenu component**

Create `frontend/src/components/ui/dropdown-menu.tsx`:
```typescript
import * as React from "react"
import * as DropdownMenuPrimitive from "@radix-ui/react-dropdown-menu"
import { Check, ChevronRight, Circle } from "lucide-react"
import { cn } from "@/lib/utils"

const DropdownMenu = DropdownMenuPrimitive.Root
const DropdownMenuTrigger = DropdownMenuPrimitive.Trigger
const DropdownMenuGroup = DropdownMenuPrimitive.Group
const DropdownMenuPortal = DropdownMenuPrimitive.Portal
const DropdownMenuSub = DropdownMenuPrimitive.Sub
const DropdownMenuRadioGroup = DropdownMenuPrimitive.RadioGroup

const DropdownMenuSubTrigger = React.forwardRef<
  React.ElementRef<typeof DropdownMenuPrimitive.SubTrigger>,
  React.ComponentPropsWithoutRef<typeof DropdownMenuPrimitive.SubTrigger> & { inset?: boolean }
>(({ className, inset, children, ...props }, ref) => (
  <DropdownMenuPrimitive.SubTrigger
    ref={ref}
    className={cn(
      "flex cursor-default select-none items-center rounded-sm px-2 py-1.5 text-sm outline-none focus:bg-accent data-[state=open]:bg-accent",
      inset && "pl-8",
      className
    )}
    {...props}
  >
    {children}
    <ChevronRight className="ml-auto h-4 w-4" />
  </DropdownMenuPrimitive.SubTrigger>
))
DropdownMenuSubTrigger.displayName = DropdownMenuPrimitive.SubTrigger.displayName

const DropdownMenuSubContent = React.forwardRef<
  React.ElementRef<typeof DropdownMenuPrimitive.SubContent>,
  React.ComponentPropsWithoutRef<typeof DropdownMenuPrimitive.SubContent>
>(({ className, ...props }, ref) => (
  <DropdownMenuPrimitive.SubContent
    ref={ref}
    className={cn(
      "z-50 min-w-[8rem] overflow-hidden rounded-md border bg-popover p-1 text-popover-foreground shadow-lg data-[state=open]:animate-in data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0 data-[state=closed]:zoom-out-95 data-[state=open]:zoom-in-95 data-[side=bottom]:slide-in-from-top-2 data-[side=left]:slide-in-from-right-2 data-[side=right]:slide-in-from-left-2 data-[side=top]:slide-in-from-bottom-2",
      className
    )}
    {...props}
  />
))
DropdownMenuSubContent.displayName = DropdownMenuPrimitive.SubContent.displayName

const DropdownMenuContent = React.forwardRef<
  React.ElementRef<typeof DropdownMenuPrimitive.Content>,
  React.ComponentPropsWithoutRef<typeof DropdownMenuPrimitive.Content>
>(({ className, sideOffset = 4, ...props }, ref) => (
  <DropdownMenuPrimitive.Portal>
    <DropdownMenuPrimitive.Content
      ref={ref}
      sideOffset={sideOffset}
      className={cn(
        "z-50 min-w-[8rem] overflow-hidden rounded-md border bg-popover p-1 text-popover-foreground shadow-md data-[state=open]:animate-in data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0 data-[state=closed]:zoom-out-95 data-[state=open]:zoom-in-95 data-[side=bottom]:slide-in-from-top-2 data-[side=left]:slide-in-from-right-2 data-[side=right]:slide-in-from-left-2 data-[side=top]:slide-in-from-bottom-2",
        className
      )}
      {...props}
    />
  </DropdownMenuPrimitive.Portal>
))
DropdownMenuContent.displayName = DropdownMenuPrimitive.Content.displayName

const DropdownMenuItem = React.forwardRef<
  React.ElementRef<typeof DropdownMenuPrimitive.Item>,
  React.ComponentPropsWithoutRef<typeof DropdownMenuPrimitive.Item> & { inset?: boolean }
>(({ className, inset, ...props }, ref) => (
  <DropdownMenuPrimitive.Item
    ref={ref}
    className={cn(
      "relative flex cursor-default select-none items-center rounded-sm px-2 py-1.5 text-sm outline-none transition-colors focus:bg-accent focus:text-accent-foreground data-[disabled]:pointer-events-none data-[disabled]:opacity-50",
      inset && "pl-8",
      className
    )}
    {...props}
  />
))
DropdownMenuItem.displayName = DropdownMenuPrimitive.Item.displayName

const DropdownMenuCheckboxItem = React.forwardRef<
  React.ElementRef<typeof DropdownMenuPrimitive.CheckboxItem>,
  React.ComponentPropsWithoutRef<typeof DropdownMenuPrimitive.CheckboxItem>
>(({ className, children, checked, ...props }, ref) => (
  <DropdownMenuPrimitive.CheckboxItem
    ref={ref}
    className={cn(
      "relative flex cursor-default select-none items-center rounded-sm py-1.5 pl-8 pr-2 text-sm outline-none transition-colors focus:bg-accent focus:text-accent-foreground data-[disabled]:pointer-events-none data-[disabled]:opacity-50",
      className
    )}
    checked={checked}
    {...props}
  >
    <span className="absolute left-2 flex h-3.5 w-3.5 items-center justify-center">
      <DropdownMenuPrimitive.ItemIndicator>
        <Check className="h-4 w-4" />
      </DropdownMenuPrimitive.ItemIndicator>
    </span>
    {children}
  </DropdownMenuPrimitive.CheckboxItem>
))
DropdownMenuCheckboxItem.displayName = DropdownMenuPrimitive.CheckboxItem.displayName

const DropdownMenuRadioItem = React.forwardRef<
  React.ElementRef<typeof DropdownMenuPrimitive.RadioItem>,
  React.ComponentPropsWithoutRef<typeof DropdownMenuPrimitive.RadioItem>
>(({ className, children, ...props }, ref) => (
  <DropdownMenuPrimitive.RadioItem
    ref={ref}
    className={cn(
      "relative flex cursor-default select-none items-center rounded-sm py-1.5 pl-8 pr-2 text-sm outline-none transition-colors focus:bg-accent focus:text-accent-foreground data-[disabled]:pointer-events-none data-[disabled]:opacity-50",
      className
    )}
    {...props}
  >
    <span className="absolute left-2 flex h-3.5 w-3.5 items-center justify-center">
      <DropdownMenuPrimitive.ItemIndicator>
        <Circle className="h-2 w-2 fill-current" />
      </DropdownMenuPrimitive.ItemIndicator>
    </span>
    {children}
  </DropdownMenuPrimitive.RadioItem>
))
DropdownMenuRadioItem.displayName = DropdownMenuPrimitive.RadioItem.displayName

const DropdownMenuLabel = React.forwardRef<
  React.ElementRef<typeof DropdownMenuPrimitive.Label>,
  React.ComponentPropsWithoutRef<typeof DropdownMenuPrimitive.Label> & { inset?: boolean }
>(({ className, inset, ...props }, ref) => (
  <DropdownMenuPrimitive.Label
    ref={ref}
    className={cn("px-2 py-1.5 text-sm font-semibold", inset && "pl-8", className)}
    {...props}
  />
))
DropdownMenuLabel.displayName = DropdownMenuPrimitive.Label.displayName

const DropdownMenuSeparator = React.forwardRef<
  React.ElementRef<typeof DropdownMenuPrimitive.Separator>,
  React.ComponentPropsWithoutRef<typeof DropdownMenuPrimitive.Separator>
>(({ className, ...props }, ref) => (
  <DropdownMenuPrimitive.Separator ref={ref} className={cn("-mx-1 my-1 h-px bg-muted", className)} {...props} />
))
DropdownMenuSeparator.displayName = DropdownMenuPrimitive.Separator.displayName

const DropdownMenuShortcut = ({ className, ...props }: React.HTMLAttributes<HTMLSpanElement>) => {
  return <span className={cn("ml-auto text-xs tracking-widest opacity-60", className)} {...props} />
}
DropdownMenuShortcut.displayName = "DropdownMenuShortcut"

export {
  DropdownMenu,
  DropdownMenuTrigger,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuCheckboxItem,
  DropdownMenuRadioItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuShortcut,
  DropdownMenuGroup,
  DropdownMenuPortal,
  DropdownMenuSub,
  DropdownMenuSubContent,
  DropdownMenuSubTrigger,
  DropdownMenuRadioGroup,
}
```

**Step 5: Commit**

```bash
git add frontend/src/components/ui/sheet.tsx frontend/src/components/ui/tabs.tsx frontend/src/components/ui/dropdown-menu.tsx frontend/package.json
git commit -m "feat(ui): add sheet, tabs, and dropdown-menu components"
```

---

## Task 6: Create UI Components Index

**Files:**
- Create: `frontend/src/components/ui/index.ts`

**Step 1: Create index file**

Create `frontend/src/components/ui/index.ts`:
```typescript
export { Button, buttonVariants } from "./button"
export { Input } from "./input"
export { Badge, badgeVariants } from "./badge"
export { Card, CardHeader, CardFooter, CardTitle, CardDescription, CardContent } from "./card"
export { Skeleton } from "./skeleton"
export { Table, TableHeader, TableBody, TableFooter, TableHead, TableRow, TableCell, TableCaption } from "./table"
export { Checkbox } from "./checkbox"
export { Sheet, SheetPortal, SheetOverlay, SheetTrigger, SheetClose, SheetContent, SheetHeader, SheetFooter, SheetTitle, SheetDescription } from "./sheet"
export { Tabs, TabsList, TabsTrigger, TabsContent } from "./tabs"
export { DropdownMenu, DropdownMenuTrigger, DropdownMenuContent, DropdownMenuItem, DropdownMenuCheckboxItem, DropdownMenuRadioItem, DropdownMenuLabel, DropdownMenuSeparator, DropdownMenuShortcut, DropdownMenuGroup, DropdownMenuPortal, DropdownMenuSub, DropdownMenuSubContent, DropdownMenuSubTrigger, DropdownMenuRadioGroup } from "./dropdown-menu"
```

**Step 2: Commit**

```bash
git add frontend/src/components/ui/index.ts
git commit -m "feat(ui): add UI components barrel export"
```

---

## Task 7: Redesign Header Component

**Files:**
- Modify: `frontend/src/components/Header.tsx`

**Step 1: Replace Header with shadcn/ui and Lucide icons**

Replace `frontend/src/components/Header.tsx`:
```typescript
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
```

**Step 2: Commit**

```bash
git add frontend/src/components/Header.tsx
git commit -m "feat(ui): redesign Header with Lucide icons and shadcn/ui styling"
```

---

## Task 8: Redesign Sidebar Component

**Files:**
- Modify: `frontend/src/components/Sidebar.tsx`

**Step 1: Replace Sidebar with shadcn/ui Sheet and Lucide icons**

Replace `frontend/src/components/Sidebar.tsx`:
```typescript
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
```

**Step 2: Commit**

```bash
git add frontend/src/components/Sidebar.tsx
git commit -m "feat(ui): redesign Sidebar with Sheet for mobile and Lucide icons"
```

---

## Task 9: Redesign Layout Component

**Files:**
- Modify: `frontend/src/components/Layout.tsx`

**Step 1: Replace Layout with updated structure**

Replace `frontend/src/components/Layout.tsx`:
```typescript
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
    <div className="min-h-screen bg-background">
      <Header />

      <div className="flex">
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
        <main className="flex-1 p-6 lg:p-8">
          <div className="max-w-7xl mx-auto">{children}</div>
        </main>
      </div>
    </div>
  )
}
```

**Step 2: Commit**

```bash
git add frontend/src/components/Layout.tsx
git commit -m "feat(ui): update Layout to use shadcn/ui Button for mobile menu"
```

---

## Task 10: Create DataTable Component

**Files:**
- Create: `frontend/src/components/DataTable.tsx`

**Step 1: Create reusable DataTable component**

Create `frontend/src/components/DataTable.tsx`:
```typescript
import {
  ColumnDef,
  flexRender,
  getCoreRowModel,
  useReactTable,
  getSortedRowModel,
  SortingState,
  getPaginationRowModel,
  getFilteredRowModel,
  ColumnFiltersState,
} from "@tanstack/react-table"
import { useState } from "react"
import { ChevronUp, ChevronDown, ChevronsUpDown } from "lucide-react"
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { cn } from "@/lib/utils"

interface DataTableProps<TData, TValue> {
  columns: ColumnDef<TData, TValue>[]
  data: TData[]
  searchKey?: string
  searchPlaceholder?: string
}

export function DataTable<TData, TValue>({
  columns,
  data,
  searchKey,
  searchPlaceholder = "Search...",
}: DataTableProps<TData, TValue>) {
  const [sorting, setSorting] = useState<SortingState>([])
  const [columnFilters, setColumnFilters] = useState<ColumnFiltersState>([])
  const [rowSelection, setRowSelection] = useState({})

  const table = useReactTable({
    data,
    columns,
    getCoreRowModel: getCoreRowModel(),
    getSortedRowModel: getSortedRowModel(),
    getPaginationRowModel: getPaginationRowModel(),
    getFilteredRowModel: getFilteredRowModel(),
    onSortingChange: setSorting,
    onColumnFiltersChange: setColumnFilters,
    onRowSelectionChange: setRowSelection,
    state: {
      sorting,
      columnFilters,
      rowSelection,
    },
  })

  return (
    <div className="space-y-4">
      {/* Search */}
      {searchKey && (
        <div className="flex items-center gap-2">
          <Input
            placeholder={searchPlaceholder}
            value={(table.getColumn(searchKey)?.getFilterValue() as string) ?? ""}
            onChange={(event) =>
              table.getColumn(searchKey)?.setFilterValue(event.target.value)
            }
            className="max-w-sm"
          />
        </div>
      )}

      {/* Table */}
      <div className="rounded-md border">
        <Table>
          <TableHeader>
            {table.getHeaderGroups().map((headerGroup) => (
              <TableRow key={headerGroup.id}>
                {headerGroup.headers.map((header) => {
                  return (
                    <TableHead key={header.id}>
                      {header.isPlaceholder ? null : (
                        <div
                          className={cn(
                            "flex items-center gap-1",
                            header.column.getCanSort() && "cursor-pointer select-none"
                          )}
                          onClick={header.column.getToggleSortingHandler()}
                        >
                          {flexRender(
                            header.column.columnDef.header,
                            header.getContext()
                          )}
                          {header.column.getCanSort() && (
                            <span className="ml-1">
                              {header.column.getIsSorted() === "asc" ? (
                                <ChevronUp className="h-4 w-4" />
                              ) : header.column.getIsSorted() === "desc" ? (
                                <ChevronDown className="h-4 w-4" />
                              ) : (
                                <ChevronsUpDown className="h-4 w-4 opacity-50" />
                              )}
                            </span>
                          )}
                        </div>
                      )}
                    </TableHead>
                  )
                })}
              </TableRow>
            ))}
          </TableHeader>
          <TableBody>
            {table.getRowModel().rows?.length ? (
              table.getRowModel().rows.map((row) => (
                <TableRow
                  key={row.id}
                  data-state={row.getIsSelected() && "selected"}
                >
                  {row.getVisibleCells().map((cell) => (
                    <TableCell key={cell.id}>
                      {flexRender(cell.column.columnDef.cell, cell.getContext())}
                    </TableCell>
                  ))}
                </TableRow>
              ))
            ) : (
              <TableRow>
                <TableCell colSpan={columns.length} className="h-24 text-center">
                  No results.
                </TableCell>
              </TableRow>
            )}
          </TableBody>
        </Table>
      </div>

      {/* Pagination */}
      <div className="flex items-center justify-between">
        <div className="text-sm text-muted-foreground">
          {table.getFilteredSelectedRowModel().rows.length} of{" "}
          {table.getFilteredRowModel().rows.length} row(s) selected.
        </div>
        <div className="flex items-center gap-2">
          <Button
            variant="outline"
            size="sm"
            onClick={() => table.previousPage()}
            disabled={!table.getCanPreviousPage()}
          >
            Previous
          </Button>
          <span className="text-sm text-muted-foreground">
            Page {table.getState().pagination.pageIndex + 1} of{" "}
            {table.getPageCount()}
          </span>
          <Button
            variant="outline"
            size="sm"
            onClick={() => table.nextPage()}
            disabled={!table.getCanNextPage()}
          >
            Next
          </Button>
        </div>
      </div>
    </div>
  )
}
```

**Step 2: Commit**

```bash
git add frontend/src/components/DataTable.tsx
git commit -m "feat(ui): add reusable DataTable component with sorting and pagination"
```

---

## Task 11: Redesign Movies Page with Table View

**Files:**
- Modify: `frontend/src/pages/Movies.tsx`
- Delete: `frontend/src/components/MovieCard.tsx`

**Step 1: Replace Movies page with table view**

Replace `frontend/src/pages/Movies.tsx`:
```typescript
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
      accessorKey: "year",
      header: "Year",
      cell: ({ row }) => (
        <span className="text-muted-foreground">{row.getValue("year") || "—"}</span>
      ),
    },
    {
      accessorKey: "audio_language",
      header: "Audio",
      cell: ({ row }) => (
        <Badge variant="outline">{row.getValue("audio_language") || "Unknown"}</Badge>
      ),
    },
    {
      accessorKey: "subtitles",
      header: "Subtitles",
      cell: ({ row }) => {
        const subtitles = row.original.subtitles || []
        if (subtitles.length === 0) {
          return <span className="text-muted-foreground">—</span>
        }
        return (
          <div className="flex gap-1 flex-wrap">
            {subtitles.slice(0, 3).map((sub) => (
              <Badge key={sub.id} variant="secondary" className="text-xs">
                {sub.language}
              </Badge>
            ))}
            {subtitles.length > 3 && (
              <Badge variant="outline" className="text-xs">
                +{subtitles.length - 3}
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
        const hasSubtitles = (row.original.subtitles?.length || 0) > 0
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
```

**Step 2: Delete MovieCard component**

Run:
```bash
rm /home/aldo/Dev/aldoborrero/submate/.worktrees/submate-ui/frontend/src/components/MovieCard.tsx
```

**Step 3: Commit**

```bash
git add frontend/src/pages/Movies.tsx
git rm frontend/src/components/MovieCard.tsx
git commit -m "feat(ui): convert Movies page from grid to table view"
```

---

## Task 12: Redesign Series Page with Table View

**Files:**
- Modify: `frontend/src/pages/Series.tsx`
- Delete: `frontend/src/components/SeriesCard.tsx`

**Step 1: Replace Series page with table view**

Replace `frontend/src/pages/Series.tsx`:
```typescript
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
      accessorKey: "title",
      header: "Title",
      cell: ({ row }) => (
        <Link
          to={`/series/${row.original.id}`}
          className="font-medium hover:text-primary transition-colors"
        >
          {row.getValue("title")}
        </Link>
      ),
    },
    {
      accessorKey: "season_count",
      header: "Seasons",
      cell: ({ row }) => (
        <span className="text-muted-foreground">{row.original.season_count || 0}</span>
      ),
    },
    {
      accessorKey: "episode_count",
      header: "Episodes",
      cell: ({ row }) => (
        <span className="text-muted-foreground">{row.original.episode_count || 0}</span>
      ),
    },
    {
      id: "subtitles",
      header: "Subtitles",
      cell: ({ row }) => {
        const total = row.original.episode_count || 0
        const withSubs = row.original.episodes_with_subtitles || 0
        if (total === 0) return <span className="text-muted-foreground">—</span>
        return (
          <span className="text-muted-foreground">
            {withSubs}/{total}
          </span>
        )
      },
    },
    {
      id: "status",
      header: "Status",
      cell: ({ row }) => {
        const total = row.original.episode_count || 0
        const withSubs = row.original.episodes_with_subtitles || 0
        if (total === 0) {
          return <Badge variant="outline">No episodes</Badge>
        }
        if (withSubs === total) {
          return (
            <Badge variant="success" className="gap-1">
              <Check className="h-3 w-3" />
              Ready
            </Badge>
          )
        }
        if (withSubs > 0) {
          return (
            <Badge variant="warning" className="gap-1">
              <AlertTriangle className="h-3 w-3" />
              Partial
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
```

**Step 2: Delete SeriesCard component**

Run:
```bash
rm /home/aldo/Dev/aldoborrero/submate/.worktrees/submate-ui/frontend/src/components/SeriesCard.tsx
```

**Step 3: Commit**

```bash
git add frontend/src/pages/Series.tsx
git rm frontend/src/components/SeriesCard.tsx
git commit -m "feat(ui): convert Series page from grid to table view"
```

---

## Task 13: Redesign Dashboard Page

**Files:**
- Modify: `frontend/src/pages/Dashboard.tsx`
- Delete: `frontend/src/components/StatsCard.tsx`

**Step 1: Replace Dashboard with shadcn/ui Card components**

Replace `frontend/src/pages/Dashboard.tsx`:
```typescript
import { useState, useEffect } from 'react'
import { Library, Film, Clock, XCircle, RefreshCw, Loader2 } from 'lucide-react'
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
  const [syncing, setSyncing] = useState(false)

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
        setSyncing(false)
      }
    })
    return unsubscribe
  }, [])

  const handleSync = async () => {
    setSyncing(true)
    try {
      await librariesApi.sync()
    } catch (error) {
      console.error('Sync failed:', error)
      setSyncing(false)
    }
  }

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
        <div className="flex items-center justify-between">
          <div>
            <Skeleton className="h-8 w-32" />
            <Skeleton className="h-4 w-48 mt-2" />
          </div>
          <Skeleton className="h-10 w-32" />
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
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold">Dashboard</h1>
          <p className="text-muted-foreground mt-1">Overview of your Submate instance</p>
        </div>
        <Button onClick={handleSync} disabled={syncing}>
          {syncing ? (
            <>
              <Loader2 className="h-4 w-4 mr-2 animate-spin" />
              Syncing...
            </>
          ) : (
            <>
              <RefreshCw className="h-4 w-4 mr-2" />
              Sync Jellyfin
            </>
          )}
        </Button>
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
                      <p className="text-sm text-muted-foreground">{job.job_type}</p>
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
```

**Step 2: Delete StatsCard component**

Run:
```bash
rm /home/aldo/Dev/aldoborrero/submate/.worktrees/submate-ui/frontend/src/components/StatsCard.tsx
```

**Step 3: Commit**

```bash
git add frontend/src/pages/Dashboard.tsx
git rm frontend/src/components/StatsCard.tsx
git commit -m "feat(ui): redesign Dashboard with shadcn/ui Card components"
```

---

## Task 14: Redesign Queue Page

**Files:**
- Modify: `frontend/src/pages/Queue.tsx`
- Delete: `frontend/src/components/JobItem.tsx`

**Step 1: Replace Queue page with table view**

Replace `frontend/src/pages/Queue.tsx`:
```typescript
import { useState, useEffect, useCallback } from 'react'
import { ColumnDef } from '@tanstack/react-table'
import { Loader2, XCircle, Check, Clock, RotateCcw, X } from 'lucide-react'
import { jobsApi, subscribeToEvents } from '@/api'
import type { Job, JobStatus, SSEEvent } from '@/api'
import { DataTable } from '@/components/DataTable'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { Skeleton } from '@/components/ui/skeleton'
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs'

type TabFilter = 'all' | JobStatus

export function QueuePage() {
  const [jobs, setJobs] = useState<Job[]>([])
  const [counts, setCounts] = useState({
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

      if (activeTab === 'all') {
        const newCounts = { pending: 0, running: 0, completed: 0, failed: 0 }
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

  const fetchCounts = useCallback(async () => {
    try {
      const response = await jobsApi.list({ page_size: 1000 })
      const newCounts = { pending: 0, running: 0, completed: 0, failed: 0 }
      response.jobs.forEach((job) => {
        newCounts[job.status]++
      })
      setCounts(newCounts)
    } catch (error) {
      console.error('Failed to fetch job counts:', error)
    }
  }, [])

  useEffect(() => {
    setLoading(true)
    fetchJobs()
  }, [fetchJobs])

  useEffect(() => {
    fetchCounts()
  }, [fetchCounts])

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

  const getStatusIcon = (status: JobStatus) => {
    switch (status) {
      case 'running': return <Loader2 className="h-3 w-3 animate-spin" />
      case 'pending': return <Clock className="h-3 w-3" />
      case 'completed': return <Check className="h-3 w-3" />
      case 'failed': return <XCircle className="h-3 w-3" />
    }
  }

  const getStatusVariant = (status: JobStatus) => {
    switch (status) {
      case 'running': return 'default'
      case 'pending': return 'secondary'
      case 'completed': return 'success'
      case 'failed': return 'destructive'
    }
  }

  const columns: ColumnDef<Job>[] = [
    {
      accessorKey: "status",
      header: "Status",
      cell: ({ row }) => (
        <Badge variant={getStatusVariant(row.original.status)} className="gap-1">
          {getStatusIcon(row.original.status)}
          {row.original.status}
        </Badge>
      ),
    },
    {
      accessorKey: "item_title",
      header: "Item",
      cell: ({ row }) => (
        <span className="font-medium">{row.getValue("item_title")}</span>
      ),
    },
    {
      accessorKey: "job_type",
      header: "Type",
      cell: ({ row }) => (
        <Badge variant="outline">{row.getValue("job_type")}</Badge>
      ),
    },
    {
      accessorKey: "progress",
      header: "Progress",
      cell: ({ row }) => {
        const progress = row.original.progress
        if (progress === undefined || progress === null) return <span className="text-muted-foreground">—</span>
        return <span>{Math.round(progress)}%</span>
      },
    },
    {
      accessorKey: "created_at",
      header: "Started",
      cell: ({ row }) => {
        const date = new Date(row.original.created_at)
        return <span className="text-muted-foreground">{date.toLocaleString()}</span>
      },
    },
    {
      id: "actions",
      cell: ({ row }) => {
        const job = row.original
        return (
          <div className="flex gap-2">
            {job.status === 'failed' && (
              <Button variant="ghost" size="sm" onClick={() => handleRetry(job.id)}>
                <RotateCcw className="h-4 w-4 mr-1" />
                Retry
              </Button>
            )}
            {job.status === 'pending' && (
              <Button variant="ghost" size="sm" onClick={() => handleCancel(job.id)}>
                <X className="h-4 w-4 mr-1" />
                Cancel
              </Button>
            )}
          </div>
        )
      },
    },
  ]

  const tabs = [
    { id: 'all' as TabFilter, label: 'All', count: counts.pending + counts.running + counts.completed + counts.failed },
    { id: 'pending' as TabFilter, label: 'Pending', count: counts.pending },
    { id: 'running' as TabFilter, label: 'Running', count: counts.running },
    { id: 'completed' as TabFilter, label: 'Completed', count: counts.completed },
    { id: 'failed' as TabFilter, label: 'Failed', count: counts.failed },
  ]

  if (loading) {
    return (
      <div className="space-y-6">
        <div>
          <Skeleton className="h-8 w-24" />
          <Skeleton className="h-4 w-48 mt-2" />
        </div>
        <Skeleton className="h-10 w-full max-w-md" />
        <Skeleton className="h-[400px]" />
      </div>
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
      <Tabs value={activeTab} onValueChange={(v) => setActiveTab(v as TabFilter)}>
        <TabsList>
          {tabs.map((tab) => (
            <TabsTrigger key={tab.id} value={tab.id} className="gap-2">
              {tab.label}
              <Badge variant="secondary" className="ml-1">
                {tab.count}
              </Badge>
            </TabsTrigger>
          ))}
        </TabsList>
      </Tabs>

      {/* Table */}
      <DataTable columns={columns} data={jobs} />
    </div>
  )
}
```

**Step 2: Delete JobItem component**

Run:
```bash
rm /home/aldo/Dev/aldoborrero/submate/.worktrees/submate-ui/frontend/src/components/JobItem.tsx
```

**Step 3: Commit**

```bash
git add frontend/src/pages/Queue.tsx
git rm frontend/src/components/JobItem.tsx
git commit -m "feat(ui): redesign Queue page with table view and shadcn/ui Tabs"
```

---

## Task 15: Redesign Settings Page

**Files:**
- Modify: `frontend/src/pages/Settings.tsx`

**Step 1: Replace Settings page with shadcn/ui components**

Replace `frontend/src/pages/Settings.tsx`:
```typescript
import { useState, useEffect } from 'react'
import { Loader2, CheckCircle, XCircle } from 'lucide-react'
import { settingsApi } from '@/api'
import type {
  Settings,
  JellyfinSettings,
  WhisperSettings,
  TranslationSettings,
  NotificationSettings,
  TestConnectionResponse,
} from '@/api'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { Skeleton } from '@/components/ui/skeleton'
import { cn } from '@/lib/utils'

export function SettingsPage() {
  const [settings, setSettings] = useState<Settings | null>(null)
  const [loading, setLoading] = useState(true)
  const [saving, setSaving] = useState(false)
  const [testResult, setTestResult] = useState<TestConnectionResponse | null>(null)
  const [testing, setTesting] = useState(false)

  useEffect(() => {
    async function fetchSettings() {
      try {
        const response = await settingsApi.get()
        setSettings(response)
      } catch (error) {
        console.error('Failed to fetch settings:', error)
      } finally {
        setLoading(false)
      }
    }
    fetchSettings()
  }, [])

  const handleSave = async () => {
    if (!settings) return
    setSaving(true)
    try {
      await settingsApi.update(settings)
    } catch (error) {
      console.error('Failed to save settings:', error)
    } finally {
      setSaving(false)
    }
  }

  const handleTestJellyfin = async () => {
    if (!settings) return
    setTesting(true)
    setTestResult(null)
    try {
      const result = await settingsApi.testJellyfin(settings.jellyfin)
      setTestResult(result)
    } catch {
      setTestResult({ success: false, message: 'Connection failed', details: {} })
    } finally {
      setTesting(false)
    }
  }

  const handleTestNotification = async () => {
    if (!settings) return
    setTesting(true)
    setTestResult(null)
    try {
      const result = await settingsApi.testNotification(settings.notifications)
      setTestResult(result)
    } catch {
      setTestResult({ success: false, message: 'Test failed', details: {} })
    } finally {
      setTesting(false)
    }
  }

  const updateJellyfin = (field: keyof JellyfinSettings, value: string) => {
    if (!settings) return
    setSettings({ ...settings, jellyfin: { ...settings.jellyfin, [field]: value } })
  }

  const updateWhisper = (field: keyof WhisperSettings, value: string) => {
    if (!settings) return
    setSettings({ ...settings, whisper: { ...settings.whisper, [field]: value } })
  }

  const updateTranslation = (field: keyof TranslationSettings, value: string) => {
    if (!settings) return
    setSettings({ ...settings, translation: { ...settings.translation, [field]: value } })
  }

  const updateNotification = (field: keyof NotificationSettings, value: string | null) => {
    if (!settings) return
    setSettings({ ...settings, notifications: { ...settings.notifications, [field]: value } })
  }

  if (loading) {
    return (
      <div className="space-y-6">
        <Skeleton className="h-8 w-32" />
        <Skeleton className="h-[600px]" />
      </div>
    )
  }

  if (!settings) {
    return <div className="text-destructive text-center py-12">Failed to load settings</div>
  }

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold">Settings</h1>
          <p className="text-muted-foreground mt-1">Configure your Submate instance</p>
        </div>
        <Button onClick={handleSave} disabled={saving}>
          {saving ? (
            <>
              <Loader2 className="h-4 w-4 mr-2 animate-spin" />
              Saving...
            </>
          ) : (
            'Save Changes'
          )}
        </Button>
      </div>

      {/* Test Result */}
      {testResult && (
        <Card className={cn(testResult.success ? "border-emerald-500" : "border-destructive")}>
          <CardContent className="flex items-center gap-2 p-4">
            {testResult.success ? (
              <CheckCircle className="h-5 w-5 text-emerald-500" />
            ) : (
              <XCircle className="h-5 w-5 text-destructive" />
            )}
            <span className={testResult.success ? "text-emerald-500" : "text-destructive"}>
              {testResult.message}
            </span>
          </CardContent>
        </Card>
      )}

      {/* Tabs */}
      <Tabs defaultValue="jellyfin" onValueChange={() => setTestResult(null)}>
        <TabsList className="grid w-full grid-cols-4">
          <TabsTrigger value="jellyfin">Jellyfin</TabsTrigger>
          <TabsTrigger value="whisper">Whisper</TabsTrigger>
          <TabsTrigger value="translation">Translation</TabsTrigger>
          <TabsTrigger value="notifications">Notifications</TabsTrigger>
        </TabsList>

        <TabsContent value="jellyfin">
          <Card>
            <CardHeader>
              <CardTitle>Jellyfin Connection</CardTitle>
              <CardDescription>Configure your Jellyfin server connection</CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="space-y-2">
                <label className="text-sm font-medium">Server URL</label>
                <Input
                  value={settings.jellyfin.server_url}
                  onChange={(e) => updateJellyfin('server_url', e.target.value)}
                  placeholder="http://jellyfin:8096"
                />
              </div>
              <div className="space-y-2">
                <label className="text-sm font-medium">API Key</label>
                <Input
                  type="password"
                  value={settings.jellyfin.api_key}
                  onChange={(e) => updateJellyfin('api_key', e.target.value)}
                  placeholder="Your Jellyfin API key"
                />
              </div>
              <Button variant="secondary" onClick={handleTestJellyfin} disabled={testing}>
                {testing ? <Loader2 className="h-4 w-4 mr-2 animate-spin" /> : null}
                Test Connection
              </Button>
            </CardContent>
          </Card>
        </TabsContent>

        <TabsContent value="whisper">
          <Card>
            <CardHeader>
              <CardTitle>Whisper Settings</CardTitle>
              <CardDescription>Configure the Whisper speech recognition model</CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="space-y-2">
                <label className="text-sm font-medium">Model</label>
                <select
                  value={settings.whisper.model}
                  onChange={(e) => updateWhisper('model', e.target.value)}
                  className="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
                >
                  <option value="tiny">Tiny (fastest)</option>
                  <option value="base">Base</option>
                  <option value="small">Small</option>
                  <option value="medium">Medium (recommended)</option>
                  <option value="large">Large (most accurate)</option>
                </select>
              </div>
              <div className="space-y-2">
                <label className="text-sm font-medium">Device</label>
                <select
                  value={settings.whisper.device}
                  onChange={(e) => updateWhisper('device', e.target.value)}
                  className="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
                >
                  <option value="auto">Auto (detect GPU)</option>
                  <option value="cpu">CPU</option>
                  <option value="cuda">CUDA (NVIDIA GPU)</option>
                </select>
              </div>
              <div className="space-y-2">
                <label className="text-sm font-medium">Compute Type</label>
                <select
                  value={settings.whisper.compute_type}
                  onChange={(e) => updateWhisper('compute_type', e.target.value)}
                  className="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
                >
                  <option value="int8">INT8 (fastest)</option>
                  <option value="float16">Float16</option>
                  <option value="float32">Float32 (most accurate)</option>
                </select>
              </div>
            </CardContent>
          </Card>
        </TabsContent>

        <TabsContent value="translation">
          <Card>
            <CardHeader>
              <CardTitle>Translation Settings</CardTitle>
              <CardDescription>Configure the LLM translation backend</CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="space-y-2">
                <label className="text-sm font-medium">Backend</label>
                <select
                  value={settings.translation.backend}
                  onChange={(e) => updateTranslation('backend', e.target.value)}
                  className="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
                >
                  <option value="ollama">Ollama (local, free)</option>
                  <option value="openai">OpenAI</option>
                  <option value="anthropic">Anthropic Claude</option>
                  <option value="gemini">Google Gemini</option>
                </select>
              </div>

              {settings.translation.backend === 'ollama' && (
                <>
                  <div className="space-y-2">
                    <label className="text-sm font-medium">Ollama URL</label>
                    <Input
                      value={settings.translation.ollama_url}
                      onChange={(e) => updateTranslation('ollama_url', e.target.value)}
                      placeholder="http://localhost:11434"
                    />
                  </div>
                  <div className="space-y-2">
                    <label className="text-sm font-medium">Ollama Model</label>
                    <Input
                      value={settings.translation.ollama_model}
                      onChange={(e) => updateTranslation('ollama_model', e.target.value)}
                      placeholder="llama2"
                    />
                  </div>
                </>
              )}

              {settings.translation.backend === 'openai' && (
                <>
                  <div className="space-y-2">
                    <label className="text-sm font-medium">OpenAI API Key</label>
                    <Input
                      type="password"
                      value={settings.translation.openai_api_key}
                      onChange={(e) => updateTranslation('openai_api_key', e.target.value)}
                    />
                  </div>
                  <div className="space-y-2">
                    <label className="text-sm font-medium">OpenAI Model</label>
                    <Input
                      value={settings.translation.openai_model}
                      onChange={(e) => updateTranslation('openai_model', e.target.value)}
                      placeholder="gpt-4"
                    />
                  </div>
                </>
              )}

              {settings.translation.backend === 'anthropic' && (
                <>
                  <div className="space-y-2">
                    <label className="text-sm font-medium">Anthropic API Key</label>
                    <Input
                      type="password"
                      value={settings.translation.anthropic_api_key}
                      onChange={(e) => updateTranslation('anthropic_api_key', e.target.value)}
                    />
                  </div>
                  <div className="space-y-2">
                    <label className="text-sm font-medium">Claude Model</label>
                    <Input
                      value={settings.translation.claude_model}
                      onChange={(e) => updateTranslation('claude_model', e.target.value)}
                      placeholder="claude-3-sonnet-20240229"
                    />
                  </div>
                </>
              )}

              {settings.translation.backend === 'gemini' && (
                <>
                  <div className="space-y-2">
                    <label className="text-sm font-medium">Gemini API Key</label>
                    <Input
                      type="password"
                      value={settings.translation.gemini_api_key}
                      onChange={(e) => updateTranslation('gemini_api_key', e.target.value)}
                    />
                  </div>
                  <div className="space-y-2">
                    <label className="text-sm font-medium">Gemini Model</label>
                    <Input
                      value={settings.translation.gemini_model}
                      onChange={(e) => updateTranslation('gemini_model', e.target.value)}
                      placeholder="gemini-pro"
                    />
                  </div>
                </>
              )}
            </CardContent>
          </Card>
        </TabsContent>

        <TabsContent value="notifications">
          <Card>
            <CardHeader>
              <CardTitle>Notification Settings</CardTitle>
              <CardDescription>Configure how you receive notifications</CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="space-y-2">
                <label className="text-sm font-medium">Webhook URL</label>
                <Input
                  value={settings.notifications.webhook_url || ''}
                  onChange={(e) => updateNotification('webhook_url', e.target.value || null)}
                  placeholder="https://your-webhook.url/endpoint"
                />
              </div>
              <div className="grid grid-cols-2 gap-4">
                <div className="space-y-2">
                  <label className="text-sm font-medium">ntfy URL</label>
                  <Input
                    value={settings.notifications.ntfy_url || ''}
                    onChange={(e) => updateNotification('ntfy_url', e.target.value || null)}
                    placeholder="https://ntfy.sh"
                  />
                </div>
                <div className="space-y-2">
                  <label className="text-sm font-medium">ntfy Topic</label>
                  <Input
                    value={settings.notifications.ntfy_topic || ''}
                    onChange={(e) => updateNotification('ntfy_topic', e.target.value || null)}
                    placeholder="submate"
                  />
                </div>
              </div>
              <Button variant="secondary" onClick={handleTestNotification} disabled={testing}>
                {testing ? <Loader2 className="h-4 w-4 mr-2 animate-spin" /> : null}
                Send Test Notification
              </Button>
            </CardContent>
          </Card>
        </TabsContent>
      </Tabs>
    </div>
  )
}
```

**Step 2: Commit**

```bash
git add frontend/src/pages/Settings.tsx
git commit -m "feat(ui): redesign Settings page with shadcn/ui Tabs and Card"
```

---

## Task 16: Update Component Exports

**Files:**
- Modify: `frontend/src/components/index.ts`

**Step 1: Update component barrel export**

Replace `frontend/src/components/index.ts`:
```typescript
export { Header } from './Header'
export { Sidebar } from './Sidebar'
export { Layout } from './Layout'
export { DataTable } from './DataTable'
export { SubtitleEditor } from './SubtitleEditor'
export { EpisodeList } from './EpisodeList'
```

**Step 2: Commit**

```bash
git add frontend/src/components/index.ts
git commit -m "chore(ui): update component exports after redesign"
```

---

## Task 17: Verify Build and Test

**Step 1: Run type check**

Run:
```bash
cd /home/aldo/Dev/aldoborrero/submate/.worktrees/submate-ui/frontend && bun run type-check
```
Expected: No TypeScript errors

**Step 2: Run build**

Run:
```bash
cd /home/aldo/Dev/aldoborrero/submate/.worktrees/submate-ui/frontend && bun run build
```
Expected: Build completes successfully

**Step 3: Commit any fixes if needed**

If there are errors, fix them and commit:
```bash
git add -A
git commit -m "fix(ui): resolve build errors from redesign"
```

---

## Summary

This plan transforms the Submate UI from a custom Tailwind implementation to a polished shadcn/ui-based interface matching the *arr family aesthetic:

| Task | Description |
|------|-------------|
| 1 | Install shadcn/ui dependencies |
| 2 | Set up teal theme and utils |
| 3 | Add core UI components |
| 4 | Add table components |
| 5 | Add navigation components |
| 6 | Create UI components index |
| 7 | Redesign Header |
| 8 | Redesign Sidebar |
| 9 | Update Layout |
| 10 | Create DataTable component |
| 11 | Redesign Movies page |
| 12 | Redesign Series page |
| 13 | Redesign Dashboard page |
| 14 | Redesign Queue page |
| 15 | Redesign Settings page |
| 16 | Update component exports |
| 17 | Verify build |

**Files removed:**
- `MovieCard.tsx` (replaced by table row)
- `SeriesCard.tsx` (replaced by table row)
- `StatsCard.tsx` (replaced by shadcn Card)
- `JobItem.tsx` (replaced by table row)
