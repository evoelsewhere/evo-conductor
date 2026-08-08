import { useQuery } from "@tanstack/react-query"
import { Link, Outlet, useRouterState } from "@tanstack/react-router"
import { AnimatePresence, motion } from "framer-motion"
import {
  Boxes,
  KeyRound,
  LayoutDashboard,
  LogOut,
  Menu,
  PanelLeftClose,
  PanelLeftOpen,
  Settings,
  Shield,
  Tags,
  Users,
  X,
  type LucideIcon,
} from "lucide-react"
import { useEffect, useMemo } from "react"

import { api } from "@/shared/api/client"
import { BrandMark } from "@/shared/components/brand"
import { ThemeToggle } from "@/shared/components/theme-toggle"
import { useIsDesktop } from "@/shared/hooks/use-media-query"
import { cn } from "@/shared/lib/utils"
import { useAuthStore } from "@/shared/stores/auth"
import { useUiStore } from "@/shared/stores/ui"
import { Badge } from "@/shared/ui/badge"
import { Button } from "@/shared/ui/button"
import { Tooltip, TooltipProvider } from "@/shared/ui/tooltip"

type NavItemDef = {
  to: string
  label: string
  icon: LucideIcon
  end: boolean
  badge?: number
}

type NavGroup = {
  label: string
  items: NavItemDef[]
}

function isActivePath(pathname: string, to: string, end: boolean) {
  return end
    ? pathname === to
    : pathname === to || pathname.startsWith(`${to}/`)
}

export function AppShell() {
  const hydrateUi = useUiStore((s) => s.hydrate)
  const hydrateAuth = useAuthStore((s) => s.hydrate)
  const isDesktop = useIsDesktop()
  const collapsed = useUiStore((s) => s.sidebarCollapsed)
  const mobileOpen = useUiStore((s) => s.mobileNavOpen)
  const setMobileNav = useUiStore((s) => s.setMobileNav)
  const pathname = useRouterState({ select: (s) => s.location.pathname })
  const user = useAuthStore((s) => s.user)
  const isAdmin = user?.primary_role === "admin"

  const { data: pending } = useQuery({
    queryKey: ["pending-count"],
    queryFn: () => api.pendingCount(),
    enabled: isAdmin,
    refetchInterval: 30_000,
  })

  const navGroups = useMemo((): NavGroup[] => {
    const canListMembers =
      user?.primary_role === "admin" || user?.primary_role === "contribute"

    const workspaceItems: NavItemDef[] = [
      { to: "/app", label: "Overview", icon: LayoutDashboard, end: true },
    ]
    if (canListMembers) {
      workspaceItems.push({
        to: "/app/members",
        label: "Members",
        icon: Users,
        end: false,
        badge: isAdmin ? pending?.count : undefined,
      })
    }
    workspaceItems.push({
      to: "/app/resources",
      label: "Resources",
      icon: Boxes,
      end: false,
    })

    const accessItems: NavItemDef[] = [
      { to: "/app/secrets", label: "Secrets", icon: KeyRound, end: false },
    ]
    if (canListMembers) {
      accessItems.unshift({
        to: "/app/tags",
        label: "Tags",
        icon: Tags,
        end: false,
      })
    }
    if (isAdmin) {
      accessItems.unshift({
        to: "/app/roles",
        label: "Roles",
        icon: Shield,
        end: false,
      })
    }

    const groups: NavGroup[] = [
      { label: "Workspace", items: workspaceItems },
      { label: "Access", items: accessItems },
    ]
    if (isAdmin) {
      groups.push({
        label: "Admin",
        items: [
          { to: "/app/settings", label: "Settings", icon: Settings, end: false },
        ],
      })
    }
    return groups
  }, [isAdmin, pending?.count, user?.primary_role])

  useEffect(() => {
    hydrateUi()
    hydrateAuth()
  }, [hydrateUi, hydrateAuth])

  useEffect(() => {
    setMobileNav(false)
  }, [pathname, setMobileNav])

  useEffect(() => {
    if (isDesktop) setMobileNav(false)
  }, [isDesktop, setMobileNav])

  return (
    <TooltipProvider delay={400}>
      <div data-shell className="flex min-h-dvh">
        {isDesktop ? (
          <SidebarRail
            collapsed={collapsed}
            pathname={pathname}
            navGroups={navGroups}
          />
        ) : (
          <MobileDrawer
            open={mobileOpen}
            pathname={pathname}
            navGroups={navGroups}
          />
        )}

        <div className="flex min-w-0 flex-1 flex-col">
          <Topbar />
          <main className="min-h-0 flex-1 overflow-y-auto">
            <Outlet />
          </main>
        </div>
      </div>
    </TooltipProvider>
  )
}

function Topbar() {
  const isDesktop = useIsDesktop()
  const collapsed = useUiStore((s) => s.sidebarCollapsed)
  const toggleSidebar = useUiStore((s) => s.toggleSidebar)
  const mobileOpen = useUiStore((s) => s.mobileNavOpen)
  const setMobileNav = useUiStore((s) => s.setMobileNav)
  const user = useAuthStore((s) => s.user)

  return (
    <header className="sticky top-0 z-(--z-header) flex h-14 items-center gap-3 border-b border-(--border-soft) bg-(--bg-page)/80 px-4 backdrop-blur-md md:px-6">
      <Button
        variant="ghost"
        size="icon"
        aria-label={
          isDesktop
            ? collapsed
              ? "Expand sidebar"
              : "Collapse sidebar"
            : mobileOpen
              ? "Close menu"
              : "Open menu"
        }
        onClick={() => {
          if (isDesktop) toggleSidebar()
          else setMobileNav(!mobileOpen)
        }}
      >
        {isDesktop ? (
          collapsed ? (
            <PanelLeftOpen className="size-4" />
          ) : (
            <PanelLeftClose className="size-4" />
          )
        ) : mobileOpen ? (
          <X className="size-4" />
        ) : (
          <Menu className="size-4" />
        )}
      </Button>

      {!isDesktop && <BrandMark size="sm" tagline={null} className="min-w-0" />}

      <div className="ml-auto flex items-center gap-2">
        {user && (
          <div className="hidden items-center gap-2 sm:flex">
            <span className="max-w-40 truncate text-xs text-(--color-text-muted)">
              {user.email}
            </span>
            <Badge tone="accent" className="capitalize">
              {user.primary_role}
            </Badge>
          </div>
        )}
        <ThemeToggle />
      </div>
    </header>
  )
}

function SidebarRail({
  collapsed,
  pathname,
  navGroups,
}: {
  collapsed: boolean
  pathname: string
  navGroups: NavGroup[]
}) {
  return (
    <aside
      className={cn(
        "sticky top-0 flex h-dvh shrink-0 flex-col border-r border-(--sidebar-border) bg-(--bg-sidebar) transition-[width] duration-(--motion-base) ease-(--ease-out)",
        collapsed ? "w-[4.25rem]" : "w-60",
      )}
    >
      <div
        className={cn(
          "flex h-14 items-center border-b border-(--border-soft)",
          collapsed ? "justify-center px-2" : "px-4",
        )}
      >
        <BrandMark
          size="sm"
          compact={collapsed}
          tagline={collapsed ? null : "master control for EvoFlux"}
        />
      </div>

      <nav className="flex flex-1 flex-col gap-4 overflow-y-auto p-2">
        {navGroups.map((group) => (
          <div key={group.label}>
            {!collapsed && (
              <div className="mb-1 px-2.5 text-[0.65rem] font-medium tracking-wider text-(--color-text-subtle) uppercase">
                {group.label}
              </div>
            )}
            <div className="flex flex-col gap-0.5">
              {group.items.map((item) => (
                <NavItem
                  key={item.to}
                  item={item}
                  active={isActivePath(pathname, item.to, item.end)}
                  collapsed={collapsed}
                />
              ))}
            </div>
          </div>
        ))}
      </nav>

      <UserFooter collapsed={collapsed} />
    </aside>
  )
}

function MobileDrawer({
  open,
  pathname,
  navGroups,
}: {
  open: boolean
  pathname: string
  navGroups: NavGroup[]
}) {
  const setMobileNav = useUiStore((s) => s.setMobileNav)

  return (
    <AnimatePresence>
      {open && (
        <>
          <motion.button
            type="button"
            aria-label="Close menu"
            className="fixed inset-0 z-(--z-overlay) bg-(--color-overlay)"
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 0.18 }}
            onClick={() => setMobileNav(false)}
          />
          <motion.aside
            className="fixed inset-y-0 left-0 z-(--z-modal) flex w-[min(18rem,88vw)] flex-col border-r border-(--sidebar-border) bg-(--bg-sidebar) shadow-(--shadow-depth)"
            initial={{ x: "-100%" }}
            animate={{ x: 0 }}
            exit={{ x: "-100%" }}
            transition={{ duration: 0.24, ease: [0.16, 1, 0.3, 1] }}
          >
            <div className="flex h-14 items-center justify-between border-b border-(--border-soft) px-4">
              <BrandMark size="sm" tagline={null} />
              <Button
                variant="ghost"
                size="icon"
                aria-label="Close menu"
                onClick={() => setMobileNav(false)}
              >
                <X className="size-4" />
              </Button>
            </div>

            <nav className="flex flex-1 flex-col gap-4 overflow-y-auto p-2">
              {navGroups.map((group) => (
                <div key={group.label}>
                  <div className="mb-1 px-2.5 text-[0.65rem] font-medium tracking-wider text-(--color-text-subtle) uppercase">
                    {group.label}
                  </div>
                  <div className="flex flex-col gap-0.5">
                    {group.items.map((item) => (
                      <NavItem
                        key={item.to}
                        item={item}
                        active={isActivePath(pathname, item.to, item.end)}
                        collapsed={false}
                      />
                    ))}
                  </div>
                </div>
              ))}
            </nav>

            <UserFooter collapsed={false} />
          </motion.aside>
        </>
      )}
    </AnimatePresence>
  )
}

function NavItem({
  item,
  active,
  collapsed,
}: {
  item: NavItemDef
  active: boolean
  collapsed: boolean
}) {
  const badge =
    item.badge && item.badge > 0 ? (
      <span className="ml-auto rounded-full bg-(--color-warning)/20 px-1.5 text-[0.65rem] font-medium text-(--color-warning) tabular-nums">
        {item.badge > 99 ? "99+" : item.badge}
      </span>
    ) : null

  const link = (
    <Link
      to={item.to}
      aria-current={active ? "page" : undefined}
      className={cn(
        "flex items-center gap-2.5 rounded-md px-2.5 py-2 text-sm text-(--color-text-muted) transition-colors hover:bg-(--bg-key) hover:text-(--color-text)",
        collapsed && "relative justify-center px-0",
        active &&
          "arc-active-indicator bg-(--bg-key) font-medium text-(--color-text)",
      )}
    >
      <item.icon className="size-4 shrink-0 opacity-85" strokeWidth={1.65} />
      {!collapsed && <span className="truncate">{item.label}</span>}
      {!collapsed && badge}
      {collapsed && item.badge && item.badge > 0 && (
        <span className="absolute top-1 right-1 size-1.5 rounded-full bg-(--color-warning)" />
      )}
    </Link>
  )

  return (
    <Tooltip content={item.label} side="right" disabled={!collapsed}>
      {link}
    </Tooltip>
  )
}

function UserFooter({ collapsed }: { collapsed: boolean }) {
  const user = useAuthStore((s) => s.user)
  const clear = useAuthStore((s) => s.clear)

  return (
    <div
      className={cn(
        "mt-auto border-t border-(--border-soft)",
        collapsed ? "p-2" : "p-3",
      )}
    >
      {!collapsed && user && (
        <div className="mb-2 min-w-0">
          <div className="truncate text-sm font-medium text-(--color-text)">
            {user.display_name}
          </div>
          <div className="mt-1 flex items-center gap-1.5">
            <Badge tone="accent" className="capitalize">
              {user.primary_role}
            </Badge>
            <span className="truncate text-[0.7rem] text-(--color-text-subtle)">
              {user.email}
            </span>
          </div>
        </div>
      )}

      <div
        className={cn(
          "flex gap-1",
          collapsed ? "flex-col items-center" : "items-center",
        )}
      >
        <ThemeToggle
          showLabel={!collapsed}
          className={collapsed ? undefined : "flex-1"}
        />
        <Tooltip content="Sign out" side={collapsed ? "right" : "top"}>
          <Button
            variant="ghost"
            size={collapsed ? "icon" : "sm"}
            className={cn(!collapsed && "shrink-0")}
            aria-label="Sign out"
            onClick={() => {
              clear()
              window.location.href = "/login"
            }}
          >
            <LogOut className="size-3.5" />
            {!collapsed && "Sign out"}
          </Button>
        </Tooltip>
      </div>
    </div>
  )
}
