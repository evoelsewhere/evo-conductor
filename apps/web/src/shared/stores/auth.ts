import { create } from "zustand"

import {
  api,
  configureAuthorizationFailureHandler,
  type AuthorizationProjection,
  type PermissionKey,
  type User,
} from "@/shared/api/client"
import {
  AUTHORIZATION_DECISION,
  evaluatePermission,
  type AuthorizationDecision,
  type AuthorizationTargetContext,
} from "@/shared/lib/authorization"
import { authSession } from "@/shared/lib/auth-session"
import { queryClient } from "@/shared/lib/query-client"

type AuthorizationStatus = "idle" | "loading" | "ready" | "error"

interface AuthState {
  token: string | null
  user: User | null
  authorization: AuthorizationProjection | null
  authorizationStatus: AuthorizationStatus
  authorizationError: string | null
  setSession: (token: string, user: User) => void
  setAuthorization: (authorization: AuthorizationProjection) => void
  refreshAuthorization: () => Promise<AuthorizationProjection>
  can: (
    permission: PermissionKey,
    target?: Omit<AuthorizationTargetContext, "actorId">,
  ) => AuthorizationDecision
  clear: () => void
  hydrate: () => void
}

let authorizationRefresh: Promise<AuthorizationProjection> | null = null

function createPermissionEvaluator(getState: () => AuthState): AuthState["can"] {
  return (permission, target = {}) => {
    const state = getState()
    if (!state.user || state.authorizationStatus !== "ready") {
      return AUTHORIZATION_DECISION.DENY
    }
    return evaluatePermission(state.authorization, permission, {
      ...target,
      actorId: state.user.id,
    })
  }
}

export const useAuthStore = create<AuthState>((set, get) => ({
  token: null,
  user: null,
  authorization: null,
  authorizationStatus: "idle",
  authorizationError: null,
  setSession: (token, user) => {
    authSession.set(token, user)
    const current = get()
    const identityChanged =
      current.user?.id !== user.id || current.user?.primary_role !== user.primary_role
    if (identityChanged) queryClient.clear()
    set({
      token,
      user,
      ...(identityChanged
        ? {
            authorization: null,
            authorizationStatus: "idle" as const,
            authorizationError: null,
            can: createPermissionEvaluator(get),
          }
        : {}),
    })
  },
  setAuthorization: (authorization) => {
    const user = get().user
    if (!user || authorization.current_role !== user.primary_role) {
      throw new Error("Authorization policy does not match the current session.")
    }
    const previousRevision = get().authorization?.policy_revision
    if (previousRevision && previousRevision !== authorization.policy_revision) {
      queryClient.clear()
    }
    set({
      authorization,
      authorizationStatus: "ready",
      authorizationError: null,
      can: createPermissionEvaluator(get),
    })
  },
  refreshAuthorization: () => {
    if (authorizationRefresh) return authorizationRefresh
    set((state) => ({
      authorizationStatus: state.authorization ? "ready" : "loading",
      authorizationError: null,
    }))
    authorizationRefresh = (async () => {
      const user = await api.me()
      const token = get().token ?? authSession.getToken()
      if (!token) throw new Error("The current session is unavailable.")
      get().setSession(token, user)
      if (get().authorizationStatus !== "ready") {
        set({ authorizationStatus: "loading" })
      }
      return api.authorizationMe()
    })()
      .then((authorization) => {
        get().setAuthorization(authorization)
        return authorization
      })
      .catch((error: unknown) => {
        if (get().token) {
          set({
            authorization: null,
            authorizationStatus: "error",
            authorizationError:
              error instanceof Error ? error.message : "Authorization policy is unavailable.",
            can: createPermissionEvaluator(get),
          })
        }
        throw error
      })
      .finally(() => {
        authorizationRefresh = null
      })
    return authorizationRefresh
  },
  can: createPermissionEvaluator(get),
  clear: () => {
    authSession.clear()
    queryClient.clear()
    set({
      token: null,
      user: null,
      authorization: null,
      authorizationStatus: "idle",
      authorizationError: null,
      can: createPermissionEvaluator(get),
    })
  },
  hydrate: () => {
    const token = authSession.getToken()
    const raw = authSession.getUser()
    if (!token || !raw) {
      queryClient.clear()
      set({
        token: null,
        user: null,
        authorization: null,
        authorizationStatus: "idle",
        authorizationError: null,
        can: createPermissionEvaluator(get),
      })
      return
    }
    try {
      const user = JSON.parse(raw) as User
      const current = get().user
      if (
        current?.id !== user.id ||
        current?.primary_role !== user.primary_role
      ) {
        queryClient.clear()
      }
      set({
        token,
        user,
        authorization: null,
        authorizationStatus: "idle",
        authorizationError: null,
        can: createPermissionEvaluator(get),
      })
    } catch {
      authSession.clear()
      queryClient.clear()
      set({
        token: null,
        user: null,
        authorization: null,
        authorizationStatus: "idle",
        authorizationError: null,
        can: createPermissionEvaluator(get),
      })
    }
  },
}))

configureAuthorizationFailureHandler({
  onUnauthorized: () => useAuthStore.getState().clear(),
  refreshAfterForbidden: async () => {
    await useAuthStore.getState().refreshAuthorization()
  },
})
