import { create } from "zustand"

const COLLAPSED_KEY = "conductor.sidebar.collapsed"
const NAV_GROUPS_KEY = "conductor.sidebar.nav-groups"

interface UiState {
  /** Desktop rail mode; persisted so the layout survives reloads. */
  sidebarCollapsed: boolean
  /** Mobile drawer; intentionally not persisted. */
  mobileNavOpen: boolean
  /** Admin settings modal; session-only so a reload does not reopen it. */
  settingsOpen: boolean
  /**
   * Nav section ids the user has collapsed. Empty means every group is open.
   * Persisted so section preferences survive reloads.
   */
  collapsedNavGroups: string[]
  toggleSidebar: () => void
  setMobileNav: (open: boolean) => void
  setSettingsOpen: (open: boolean) => void
  toggleNavGroup: (id: string) => void
  /** Ensure a group is expanded (e.g. when its route becomes active). */
  expandNavGroup: (id: string) => void
  hydrate: () => void
}

function readCollapsedGroups(): string[] {
  try {
    const raw = localStorage.getItem(NAV_GROUPS_KEY)
    if (!raw) return []
    const parsed = JSON.parse(raw) as unknown
    return Array.isArray(parsed)
      ? parsed.filter((item): item is string => typeof item === "string")
      : []
  } catch {
    return []
  }
}

function writeCollapsedGroups(ids: string[]) {
  localStorage.setItem(NAV_GROUPS_KEY, JSON.stringify(ids))
}

export const useUiStore = create<UiState>((set, get) => ({
  sidebarCollapsed: false,
  mobileNavOpen: false,
  settingsOpen: false,
  collapsedNavGroups: [],

  toggleSidebar: () => {
    const next = !get().sidebarCollapsed
    localStorage.setItem(COLLAPSED_KEY, String(next))
    set({ sidebarCollapsed: next })
  },

  setMobileNav: (open) => set({ mobileNavOpen: open }),

  setSettingsOpen: (open) => set({ settingsOpen: open }),

  toggleNavGroup: (id) => {
    const current = get().collapsedNavGroups
    const next = current.includes(id)
      ? current.filter((item) => item !== id)
      : [...current, id]
    writeCollapsedGroups(next)
    set({ collapsedNavGroups: next })
  },

  expandNavGroup: (id) => {
    const current = get().collapsedNavGroups
    if (!current.includes(id)) return
    const next = current.filter((item) => item !== id)
    writeCollapsedGroups(next)
    set({ collapsedNavGroups: next })
  },

  hydrate: () =>
    set({
      sidebarCollapsed: localStorage.getItem(COLLAPSED_KEY) === "true",
      collapsedNavGroups: readCollapsedGroups(),
    }),
}))
