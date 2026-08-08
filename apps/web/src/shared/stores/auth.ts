import { create } from "zustand"

import type { User } from "@/shared/api/client"

interface AuthState {
  token: string | null
  user: User | null
  setSession: (token: string, user: User) => void
  clear: () => void
  hydrate: () => void
}

export const useAuthStore = create<AuthState>((set) => ({
  token: null,
  user: null,
  setSession: (token, user) => {
    localStorage.setItem("conductor.token", token)
    localStorage.setItem("conductor.user", JSON.stringify(user))
    set({ token, user })
  },
  clear: () => {
    localStorage.removeItem("conductor.token")
    localStorage.removeItem("conductor.user")
    set({ token: null, user: null })
  },
  hydrate: () => {
    const token = localStorage.getItem("conductor.token")
    const raw = localStorage.getItem("conductor.user")
    if (!token || !raw) {
      set({ token: null, user: null })
      return
    }
    try {
      set({ token, user: JSON.parse(raw) as User })
    } catch {
      set({ token: null, user: null })
    }
  },
}))
