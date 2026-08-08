import { create } from "zustand"

const COLLAPSED_KEY = "conductor.sidebar.collapsed"

interface UiState {
  /** Desktop rail mode; persisted so the layout survives reloads. */
  sidebarCollapsed: boolean
  /** Mobile drawer; intentionally not persisted. */
  mobileNavOpen: boolean
  toggleSidebar: () => void
  setMobileNav: (open: boolean) => void
  hydrate: () => void
}

export const useUiStore = create<UiState>((set, get) => ({
  sidebarCollapsed: false,
  mobileNavOpen: false,

  toggleSidebar: () => {
    const next = !get().sidebarCollapsed
    localStorage.setItem(COLLAPSED_KEY, String(next))
    set({ sidebarCollapsed: next })
  },

  setMobileNav: (open) => set({ mobileNavOpen: open }),

  hydrate: () =>
    set({ sidebarCollapsed: localStorage.getItem(COLLAPSED_KEY) === "true" }),
}))
