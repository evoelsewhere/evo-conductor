import { useQuery } from "@tanstack/react-query"
import { Link, Outlet, useRouterState } from "@tanstack/react-router"
import { AnimatePresence, motion } from "framer-motion"
import { Dialog as DialogPrimitive } from "@base-ui/react/dialog"
import {
  Bot,
  Boxes,
  ChartNoAxesCombined,
  ChevronDown,
  ChevronsUpDown,
  KeyRound,
  LayoutDashboard,
  LogOut,
  Menu as MenuIcon,
  Monitor,
  Moon,
  PanelLeftClose,
  PanelLeftOpen,
  Plug,
  Settings,
  Shield,
  Sparkles,
  Sun,
  Tags,
  Users,
  X,
  type LucideIcon,
} from "lucide-react"
import { useEffect, useMemo } from "react"

import { api } from "@/shared/api/client"
import { BrandMark } from "@/shared/components/brand"
import { ThemeToggle } from "@/shared/components/theme-toggle"
import { SettingsDialog } from "@/features/settings/components/settings-dialog"
import { useIsDesktop } from "@/shared/hooks/use-media-query"
import { cn } from "@/shared/lib/utils"
import { useAuthStore } from "@/shared/stores/auth"
import { useThemeStore, type ThemeMode } from "@/shared/stores/theme"
import { useUiStore } from "@/shared/stores/ui"
import { Avatar } from "@/shared/ui/avatar"
import { Badge } from "@/shared/ui/badge"
import { Button } from "@/shared/ui/button"
import {
  Menu,
  MenuGroup,
  MenuGroupLabel,
  MenuItem,
  MenuRadioGroup,
  MenuRadioItem,
  MenuSeparator,
} from "@/shared/ui/menu"
import { Tooltip, TooltipProvider } from "@/shared/ui/tooltip"

type NavItemDef = {
  to: string
  label: string
  icon: LucideIcon
  end: boolean
  badge?: number
  /** Nested sub-items shown under an expandable parent (e.g. Resources). */
  children?: NavItemDef[]
}

type NavGroup = {
  id: string
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
  const settingsOpen = useUiStore((s) => s.settingsOpen)
  const setSettingsOpen = useUiStore((s) => s.setSettingsOpen)
  const pathname = useRouterState({ select: (s) => s.location.pathname })
  const user = useAuthStore((s) => s.user)
  const isAdmin = user?.primary_role === "admin"

  const { data: pending } = useQuery({
    queryKey: ["pending-count"],
    queryFn: () => api.pendingCount(),
    enabled: isAdmin,
    refetchInterval: 30_000,
  })

  const { data: branding } = useQuery({
    queryKey: ["project"],
    queryFn: () => api.project(),
  })

  const brand = useMemo(() => {
    if (!branding) return { title: undefined, tagline: undefined, logoUrl: undefined }
    const display = branding.display_name?.trim() || null
    const name = branding.project_name.trim()
    return {
      title: display || name,
      tagline: display && display !== name ? name : null,
      logoUrl: branding.logo_url,
    }
  }, [branding])

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
    const resourceItems: NavItemDef[] = [
      { to: "/app/resources/plugins", label: "Plugins", icon: Plug, end: false },
      { to: "/app/resources/skills", label: "Skills", icon: Sparkles, end: false },
      { to: "/app/resources/agents", label: "Agents", icon: Bot, end: false },
    ]
    if (canListMembers) {
      resourceItems.push({
        to: "/app/resources/usage",
        label: "Usage",
        icon: ChartNoAxesCombined,
        end: false,
      })
    }
    workspaceItems.push({
      to: "/app/resources",
      label: "Resources",
      icon: Boxes,
      end: true,
      children: resourceItems,
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
      { id: "workspace", label: "Workspace", items: workspaceItems },
      { id: "access", label: "Access", items: accessItems },
    ]
    return groups
  }, [isAdmin, pending?.count, user?.primary_role])

  useEffect(() => {
    hydrateUi()
    hydrateAuth()
  }, [hydrateUi, hydrateAuth])

  // Keep the group that owns the active route open so deep links are not hidden.
  useEffect(() => {
    const ownsActive = (item: NavItemDef) =>
      isActivePath(pathname, item.to, item.end) ||
      (item.children ?? []).some((child) => isActivePath(pathname, child.to, child.end))
    const active = navGroups.find((group) => group.items.some(ownsActive))
    if (active) useUiStore.getState().expandNavGroup(active.id)
    // Expand a parent item (e.g. Resources) when one of its sub-pages is active.
    for (const group of navGroups) {
      for (const item of group.items) {
        if ((item.children ?? []).some((child) => isActivePath(pathname, child.to, child.end))) {
          useUiStore.getState().expandNavGroup(item.to)
        }
      }
    }
  }, [pathname, navGroups])

  useEffect(() => {
    setMobileNav(false)
  }, [pathname, setMobileNav])

  useEffect(() => {
    if (isDesktop) setMobileNav(false)
  }, [isDesktop, setMobileNav])

  return (
    <TooltipProvider delay={400}>
      <div data-shell className="flex min-h-dvh">
        <a
          href="#main-content"
          className="fixed top-2 left-2 z-(--z-toast) -translate-y-16 rounded-md bg-(--bg-card) px-3 py-2 text-sm font-medium shadow-(--shadow-depth) transition-transform focus:translate-y-0"
        >
          Skip to content
        </a>
        {isDesktop ? (
          <SidebarRail
            collapsed={collapsed}
            pathname={pathname}
            navGroups={navGroups}
            brand={brand}
          />
        ) : (
          <MobileDrawer
            open={mobileOpen}
            pathname={pathname}
            navGroups={navGroups}
            brand={brand}
          />
        )}

        <div className="flex min-w-0 flex-1 flex-col">
          <Topbar brand={brand} />
          <main id="main-content" className="min-h-0 flex-1 overflow-y-auto" tabIndex={-1}>
            <Outlet />
          </main>
        </div>

        {isAdmin && (
          <SettingsDialog
            open={settingsOpen}
            onClose={() => setSettingsOpen(false)}
          />
        )}
      </div>
    </TooltipProvider>
  )
}

type BrandProps = {
  title?: string | null
  tagline?: string | null
  logoUrl?: string | null
}

function Topbar({ brand }: { brand: BrandProps }) {
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
          <MenuIcon className="size-4" />
        )}
      </Button>

      {!isDesktop && (
        <BrandMark
          size="sm"
          tagline={null}
          title={brand.title}
          logoUrl={brand.logoUrl}
          className="min-w-0"
        />
      )}

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
  brand,
}: {
  collapsed: boolean
  pathname: string
  navGroups: NavGroup[]
  brand: BrandProps
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
          title={brand.title}
          tagline={brand.tagline}
          logoUrl={brand.logoUrl}
        />
      </div>

      <nav className="flex flex-1 flex-col gap-1 overflow-y-auto p-2">
        {navGroups.map((group) => (
          <NavGroupSection
            key={group.id}
            group={group}
            pathname={pathname}
            railCollapsed={collapsed}
          />
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
  brand,
}: {
  open: boolean
  pathname: string
  navGroups: NavGroup[]
  brand: BrandProps
}) {
  const setMobileNav = useUiStore((s) => s.setMobileNav)

  return (
    <DialogPrimitive.Root
      open={open}
      onOpenChange={(next) => {
        if (!next) setMobileNav(false)
      }}
    >
      <DialogPrimitive.Portal>
        <DialogPrimitive.Backdrop className="fixed inset-0 z-(--z-overlay) bg-(--color-overlay) transition-opacity duration-(--motion-fast) data-ending-style:opacity-0 data-starting-style:opacity-0" />
        <DialogPrimitive.Popup className="fixed inset-y-0 left-0 z-(--z-modal) flex w-[min(18rem,88vw)] flex-col border-r border-(--sidebar-border) bg-(--bg-sidebar) shadow-(--shadow-depth) outline-none transition-transform duration-(--motion-base) ease-(--ease-out) data-ending-style:-translate-x-full data-starting-style:-translate-x-full">
            <DialogPrimitive.Title className="sr-only">
              Project navigation
            </DialogPrimitive.Title>
            <div className="flex h-14 items-center justify-between border-b border-(--border-soft) px-4">
              <BrandMark
                size="sm"
                tagline={null}
                title={brand.title}
                logoUrl={brand.logoUrl}
              />
              <DialogPrimitive.Close
                render={
                  <Button variant="ghost" size="icon" aria-label="Close menu" />
                }
              >
                <X className="size-4" />
              </DialogPrimitive.Close>
            </div>

            <nav className="flex flex-1 flex-col gap-1 overflow-y-auto p-2">
              {navGroups.map((group) => (
                <NavGroupSection
                  key={group.id}
                  group={group}
                  pathname={pathname}
                  railCollapsed={false}
                />
              ))}
            </nav>

            <UserFooter collapsed={false} />
        </DialogPrimitive.Popup>
      </DialogPrimitive.Portal>
    </DialogPrimitive.Root>
  )
}

function NavGroupSection({
  group,
  pathname,
  railCollapsed,
}: {
  group: NavGroup
  pathname: string
  railCollapsed: boolean
}) {
  const collapsedGroups = useUiStore((s) => s.collapsedNavGroups)
  const toggleNavGroup = useUiStore((s) => s.toggleNavGroup)
  const open = !collapsedGroups.includes(group.id)
  const hasActive = group.items.some(
    (item) =>
      isActivePath(pathname, item.to, item.end) ||
      (item.children ?? []).some((child) => isActivePath(pathname, child.to, child.end)),
  )

  // Icon rail has no room for headers; always show every item.
  if (railCollapsed) {
    return (
      <div className="flex flex-col gap-0.5">
        {group.items
          .flatMap((item) => [item, ...(item.children ?? [])])
          .map((item) => (
            <NavItem
              key={item.to}
              item={item}
              active={isActivePath(pathname, item.to, item.end)}
              collapsed
            />
          ))}
      </div>
    )
  }

  return (
    <div>
      <button
        type="button"
        aria-expanded={open}
        aria-controls={`nav-group-${group.id}`}
        onClick={() => toggleNavGroup(group.id)}
        className={cn(
          "mb-0.5 flex w-full items-center gap-1 rounded-md px-2.5 py-1.5 text-left text-[0.65rem] font-medium tracking-wider text-(--color-text-subtle) uppercase transition-colors",
          "hover:bg-(--bg-key) hover:text-(--color-text-muted)",
          hasActive && !open && "text-(--color-text-muted)",
        )}
      >
        <span className="min-w-0 flex-1 truncate">{group.label}</span>
        <ChevronDown
          className={cn(
            "size-3.5 shrink-0 opacity-70 transition-transform duration-(--motion-fast)",
            !open && "-rotate-90",
          )}
          aria-hidden
        />
      </button>

      <AnimatePresence initial={false}>
        {open && (
          <motion.div
            id={`nav-group-${group.id}`}
            key="items"
            initial={{ height: 0, opacity: 0 }}
            animate={{ height: "auto", opacity: 1 }}
            exit={{ height: 0, opacity: 0 }}
            transition={{ duration: 0.18, ease: [0.16, 1, 0.3, 1] }}
            className="overflow-hidden"
          >
            <div className="flex flex-col gap-0.5 pb-1">
              {group.items.map((item) => (
                <NavItem
                  key={item.to}
                  item={item}
                  active={isActivePath(pathname, item.to, item.end)}
                  collapsed={false}
                  pathname={pathname}
                />
              ))}
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  )
}

function NavItem({
  item,
  active,
  collapsed,
  pathname,
}: {
  item: NavItemDef
  active: boolean
  collapsed: boolean
  pathname?: string
}) {
  const count = item.badge ?? 0
  const hasBadge = count > 0

  if (!collapsed && item.children && pathname !== undefined) {
    return (
      <NavItemWithChildren item={item} active={active} pathname={pathname} />
    )
  }

  const link = (
    <Link
      to={item.to}
      aria-current={active ? "page" : undefined}
      className={cn(
        "relative flex items-center gap-2.5 rounded-md px-2.5 py-2 text-sm text-(--color-text-muted) transition-colors hover:bg-(--bg-key) hover:text-(--color-text)",
        collapsed && "justify-center px-0",
        active &&
          "arc-active-indicator bg-(--bg-key) font-medium text-(--color-text)",
      )}
    >
      <item.icon className="size-4 shrink-0 opacity-85" strokeWidth={1.65} />
      {!collapsed && <span className="truncate">{item.label}</span>}
      {!collapsed && hasBadge && (
        <span className="ml-auto inline-flex h-4 min-w-4 items-center justify-center rounded-full bg-(--color-warning) px-1 text-[0.65rem] leading-none font-semibold text-(--bg-page) tabular-nums">
          {count > 99 ? "99+" : count}
        </span>
      )}
      {/* Collapsed rail has no room for a count; a ringed dot reads as "needs attention". */}
      {collapsed && hasBadge && (
        <span
          aria-hidden
          className="absolute top-1.5 right-1.5 size-2 rounded-full bg-(--color-warning) ring-2 ring-(--bg-sidebar)"
        />
      )}
    </Link>
  )

  const label = hasBadge ? `${item.label} (${count})` : item.label

  return (
    <Tooltip content={label} side="right" disabled={!collapsed}>
      {link}
    </Tooltip>
  )
}

/** Parent nav item with an expandable set of nested sub-pages (e.g. Resources). */
function NavItemWithChildren({
  item,
  active,
  pathname,
}: {
  item: NavItemDef
  active: boolean
  pathname: string
}) {
  const collapsedGroups = useUiStore((s) => s.collapsedNavGroups)
  const toggleNavGroup = useUiStore((s) => s.toggleNavGroup)
  const open = !collapsedGroups.includes(item.to)
  const children = item.children ?? []

  return (
    <div>
      <div className="relative">
        <Link
          to={item.to}
          aria-current={active ? "page" : undefined}
          className={cn(
            "relative flex items-center gap-2.5 rounded-md py-2 pr-8 pl-2.5 text-sm text-(--color-text-muted) transition-colors hover:bg-(--bg-key) hover:text-(--color-text)",
            active && "arc-active-indicator bg-(--bg-key) font-medium text-(--color-text)",
          )}
        >
          <item.icon className="size-4 shrink-0 opacity-85" strokeWidth={1.65} />
          <span className="truncate">{item.label}</span>
        </Link>
        <button
          type="button"
          aria-expanded={open}
          aria-label={`Toggle ${item.label} submenu`}
          onClick={() => toggleNavGroup(item.to)}
          className="absolute top-1/2 right-1 grid size-6 -translate-y-1/2 place-items-center rounded text-(--color-text-subtle) transition-colors hover:bg-(--bg-key) hover:text-(--color-text)"
        >
          <ChevronDown
            className={cn(
              "size-3.5 transition-transform duration-(--motion-fast)",
              !open && "-rotate-90",
            )}
            aria-hidden
          />
        </button>
      </div>

      <AnimatePresence initial={false}>
        {open && (
          <motion.div
            key="sub-items"
            initial={{ height: 0, opacity: 0 }}
            animate={{ height: "auto", opacity: 1 }}
            exit={{ height: 0, opacity: 0 }}
            transition={{ duration: 0.18, ease: [0.16, 1, 0.3, 1] }}
            className="overflow-hidden"
          >
            <div className="mt-0.5 ml-[1.15rem] flex flex-col gap-0.5 border-l border-(--border-soft) pl-2">
              {children.map((child) => {
                const childActive = isActivePath(pathname, child.to, child.end)
                return (
                  <Link
                    key={child.to}
                    to={child.to}
                    aria-current={childActive ? "page" : undefined}
                    className={cn(
                      "flex items-center gap-2 rounded-md px-2.5 py-1.5 text-[0.8rem] text-(--color-text-muted) transition-colors hover:bg-(--bg-key) hover:text-(--color-text)",
                      childActive && "bg-(--bg-key) font-medium text-(--color-text)",
                    )}
                  >
                    <child.icon className="size-3.5 shrink-0 opacity-85" strokeWidth={1.65} />
                    <span className="truncate">{child.label}</span>
                  </Link>
                )
              })}
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  )
}

const themeOptions = [
  { value: "light", label: "Light", icon: Sun },
  { value: "dark", label: "Dark", icon: Moon },
  { value: "system", label: "System", icon: Monitor },
] as const

function UserFooter({ collapsed }: { collapsed: boolean }) {
  const user = useAuthStore((s) => s.user)
  const clear = useAuthStore((s) => s.clear)
  const isAdmin = user?.primary_role === "admin"
  const setSettingsOpen = useUiStore((s) => s.setSettingsOpen)
  const setMobileNav = useUiStore((s) => s.setMobileNav)
  const themeMode = useThemeStore((s) => s.mode)
  const setThemeMode = useThemeStore((s) => s.setMode)

  if (!user) return null

  const trigger = (
    <button
      type="button"
      aria-label="Account menu"
      title={collapsed ? user.display_name : undefined}
      className={cn(
        "flex items-center rounded-lg text-left transition-colors outline-none hover:bg-(--bg-key) focus-visible:ring-2 focus-visible:ring-(--focus-ring)/40 data-popup-open:bg-(--bg-key)",
        collapsed ? "size-10 justify-center" : "w-full gap-2.5 px-2 py-2",
      )}
    >
      <Avatar
        name={user.display_name}
        email={user.email}
        size={collapsed ? "sm" : "md"}
      />
      {!collapsed && (
        <>
          <span className="min-w-0 flex-1 leading-tight">
            <span className="block truncate text-sm font-medium text-(--color-text)">
              {user.display_name}
            </span>
            <span className="block truncate text-[0.7rem] text-(--color-text-subtle)">
              {user.email}
            </span>
          </span>
          <ChevronsUpDown className="size-3.5 shrink-0 text-(--color-text-subtle)" />
        </>
      )}
    </button>
  )

  return (
    <div className="mt-auto border-t border-(--border-soft) p-2">
      <Menu
        side="top"
        align={collapsed ? "center" : "start"}
        trigger={trigger}
      >
        <div className="flex items-center gap-2.5 px-2 py-2">
          <Avatar name={user.display_name} email={user.email} />
          <div className="min-w-0 leading-tight">
            <div className="truncate text-sm font-medium text-(--color-text)">
              {user.display_name}
            </div>
            <div className="truncate text-[0.7rem] text-(--color-text-subtle)">
              {user.email}
            </div>
          </div>
          <Badge tone="accent" className="ml-auto shrink-0 capitalize">
            {user.primary_role}
          </Badge>
        </div>

        <MenuSeparator />

        {isAdmin && (
          <MenuItem
            onClick={() => {
              setSettingsOpen(true)
              setMobileNav(false)
            }}
          >
            <Settings className="size-4 opacity-80" strokeWidth={1.7} />
            Project settings
          </MenuItem>
        )}

        <MenuGroup>
          <MenuGroupLabel>Appearance</MenuGroupLabel>
          <MenuRadioGroup
            value={themeMode}
            onValueChange={(next) => setThemeMode(next as ThemeMode)}
          >
            {themeOptions.map((option) => (
              <MenuRadioItem key={option.value} value={option.value}>
                <option.icon className="size-4 opacity-80" strokeWidth={1.7} />
                {option.label}
              </MenuRadioItem>
            ))}
          </MenuRadioGroup>
        </MenuGroup>

        <MenuSeparator />

        <MenuItem
          tone="danger"
          onClick={() => {
            clear()
            window.location.href = "/login"
          }}
        >
          <LogOut className="size-4" strokeWidth={1.7} />
          Sign out
        </MenuItem>
      </Menu>
    </div>
  )
}
