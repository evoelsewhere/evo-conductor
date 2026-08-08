import { create } from "zustand"

export type ThemeMode = "light" | "dark" | "system"

const STORAGE_KEY = "conductor.theme"

const prefersDark = () =>
  typeof window !== "undefined" &&
  window.matchMedia("(prefers-color-scheme: dark)").matches

function resolve(mode: ThemeMode): "light" | "dark" {
  return mode === "system" ? (prefersDark() ? "dark" : "light") : mode
}

function apply(mode: ThemeMode) {
  const resolved = resolve(mode)
  const root = document.documentElement
  root.classList.toggle("dark", resolved === "dark")
  root.classList.toggle("light", resolved === "light")
  root.style.colorScheme = resolved
}

function readStored(): ThemeMode {
  const raw = localStorage.getItem(STORAGE_KEY)
  return raw === "light" || raw === "dark" || raw === "system" ? raw : "system"
}

interface ThemeState {
  mode: ThemeMode
  resolved: "light" | "dark"
  setMode: (mode: ThemeMode) => void
  /** Cycles light → dark → system, matching the toggle button order. */
  cycle: () => void
  init: () => () => void
}

export const useThemeStore = create<ThemeState>((set, get) => ({
  mode: "system",
  resolved: "dark",

  setMode: (mode) => {
    localStorage.setItem(STORAGE_KEY, mode)
    apply(mode)
    set({ mode, resolved: resolve(mode) })
  },

  cycle: () => {
    const order: ThemeMode[] = ["light", "dark", "system"]
    const next = order[(order.indexOf(get().mode) + 1) % order.length]
    get().setMode(next)
  },

  init: () => {
    const mode = readStored()
    apply(mode)
    set({ mode, resolved: resolve(mode) })

    const media = window.matchMedia("(prefers-color-scheme: dark)")
    const onChange = () => {
      if (get().mode !== "system") return
      apply("system")
      set({ resolved: resolve("system") })
    }
    media.addEventListener("change", onChange)
    return () => media.removeEventListener("change", onChange)
  },
}))
