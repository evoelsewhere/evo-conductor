import { create } from "zustand"

import type { User } from "@/shared/api/client"
import { authSession } from "@/shared/lib/auth-session"

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
    authSession.set(token, user)
    set({ token, user })
  },
  clear: () => {
    authSession.clear()
    set({ token: null, user: null })
  },
  hydrate: () => {
    const token = authSession.getToken()
    const raw = authSession.getUser()
    if (!token || !raw) {
      set({ token: null, user: null })
      return
    }
    try {
      set({ token, user: JSON.parse(raw) as User })
    } catch {
      authSession.clear()
      set({ token: null, user: null })
    }
  },
}))
