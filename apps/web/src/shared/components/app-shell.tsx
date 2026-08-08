import { Link, Outlet, useRouterState } from "@tanstack/react-router"
import {
  Boxes,
  KeyRound,
  LayoutDashboard,
  LogOut,
  Shield,
  Users,
} from "lucide-react"

import { BrandMark } from "@/shared/components/brand"
import { Button } from "@/shared/ui/button"
import { Badge } from "@/shared/ui/badge"
import { useAuthStore } from "@/shared/stores/auth"
import { cn } from "@/shared/lib/utils"

const nav = [
  { to: "/app", label: "Overview", icon: LayoutDashboard, end: true },
  { to: "/app/members", label: "Members", icon: Users, end: false },
  { to: "/app/resources", label: "Resources", icon: Boxes, end: false },
  { to: "/app/secrets", label: "Secrets", icon: KeyRound, end: false },
  { to: "/app/roles", label: "Roles", icon: Shield, end: false },
] as const

export function AppShell() {
  const user = useAuthStore((s) => s.user)
  const clear = useAuthStore((s) => s.clear)
  const pathname = useRouterState({ select: (s) => s.location.pathname })

  return (
    <div className="flex min-h-screen">
      <aside className="flex w-56 shrink-0 flex-col border-r border-(--sidebar-border) bg-(--bg-sidebar)">
        <div className="border-b border-(--border-soft) px-4 py-4">
          <BrandMark />
        </div>
        <nav className="flex flex-1 flex-col gap-0.5 p-2">
          {nav.map((item) => {
            const isActive = item.end
              ? pathname === item.to
              : pathname === item.to || pathname.startsWith(`${item.to}/`)
            return (
              <Link
                key={item.to}
                to={item.to}
                className={cn(
                  "flex items-center gap-2 rounded-md px-2.5 py-2 text-sm text-(--color-text-muted) transition-colors hover:bg-(--bg-key) hover:text-(--color-text)",
                  isActive &&
                    "arc-active-indicator bg-(--bg-key) font-medium text-(--color-text)",
                )}
              >
                <item.icon className="size-4 opacity-80" strokeWidth={1.65} />
                {item.label}
              </Link>
            )
          })}
        </nav>
        <div className="border-t border-(--border-soft) p-3">
          <div className="mb-2 truncate text-sm font-medium text-(--color-text)">
            {user?.display_name}
          </div>
          <div className="mb-3 flex items-center gap-1.5">
            <Badge className="capitalize">{user?.primary_role}</Badge>
            <span className="truncate text-[0.7rem] text-(--color-text-subtle)">
              {user?.email}
            </span>
          </div>
          <Button
            variant="ghost"
            size="sm"
            className="w-full justify-start"
            onClick={() => {
              clear()
              window.location.href = "/login"
            }}
          >
            <LogOut className="size-3.5" />
            Sign out
          </Button>
        </div>
      </aside>
      <main className="min-w-0 flex-1">
        <Outlet />
      </main>
    </div>
  )
}
