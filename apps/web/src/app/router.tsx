import {
  createRootRoute,
  createRoute,
  createRouter,
  redirect,
  Outlet,
} from "@tanstack/react-router"
import { useEffect, useState } from "react"

import { api, type SetupStatus } from "@/shared/api/client"
import { AppShell } from "@/shared/components/app-shell"
import { ChangePasswordPage } from "@/features/auth/pages/change-password-page"
import { LoginPage } from "@/features/auth/pages/login-page"
import { PendingPage } from "@/features/auth/pages/pending-page"
import { SsoCallbackPage } from "@/features/auth/pages/sso-callback-page"
import { MembersPage } from "@/features/members/pages/members-page"
import { OverviewPage } from "@/features/dashboard/pages/overview-page"
import { ResourcesPage } from "@/features/resources/pages/resources-page"
import { RolesPage } from "@/features/roles/pages/roles-page"
import { SecretsPage } from "@/features/secrets/pages/secrets-page"
import { SetupPage } from "@/features/setup/pages/setup-page"
import { TagsPage } from "@/features/tags/pages/tags-page"
import { useAuthStore } from "@/shared/stores/auth"
import { authSession } from "@/shared/lib/auth-session"

function RootComponent() {
  return <Outlet />
}

const rootRoute = createRootRoute({
  component: RootComponent,
})

const indexRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/",
  component: BootGate,
})

const setupRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/setup",
  component: SetupPage,
  beforeLoad: async () => {
    const status = await api.setupStatus()
    if (status.configured) {
      throw redirect({ to: "/login" })
    }
  },
})

const loginRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/login",
  component: LoginRoute,
  beforeLoad: async () => {
    const status = await api.setupStatus()
    if (!status.configured) {
      throw redirect({ to: "/setup" })
    }
    return { status }
  },
})

const ssoCallbackRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/auth/callback",
  component: SsoCallbackPage,
})

const pendingRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/pending",
  component: PendingPage,
})

const changePasswordRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/change-password",
  component: ChangePasswordPage,
  beforeLoad: () => {
    const token = authSession.getToken()
    if (!token) throw redirect({ to: "/login" })
  },
})

function LoginRoute() {
  const { status } = loginRoute.useRouteContext() as { status: SetupStatus }
  return (
    <LoginPage
      projectName={status.project_name}
      displayName={status.display_name}
      logoUrl={status.logo_url}
      ssoEnabled={status.sso_enabled}
    />
  )
}

const appRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/app",
  component: AppShell,
  beforeLoad: async () => {
    const status = await api.setupStatus()
    if (!status.configured) throw redirect({ to: "/setup" })
    const token = authSession.getToken()
    if (!token) throw redirect({ to: "/login" })

    try {
      const user = await api.me()
      useAuthStore.getState().setSession(token, user)
      if (user.must_change_password) {
        throw redirect({ to: "/change-password" })
      }
    } catch (e) {
      if (e && typeof e === "object" && "to" in e) throw e
      useAuthStore.getState().clear()
      throw redirect({ to: "/login" })
    }
  },
})

const overviewRoute = createRoute({
  getParentRoute: () => appRoute,
  path: "/",
  component: OverviewPage,
})

const membersRoute = createRoute({
  getParentRoute: () => appRoute,
  path: "/members",
  component: MembersPage,
  beforeLoad: () => {
    const role = storedPrimaryRole()
    if (role !== "admin" && role !== "contribute") {
      throw redirect({ to: "/app" })
    }
  },
})

const resourcesRoute = createRoute({
  getParentRoute: () => appRoute,
  path: "/resources",
  component: ResourcesPage,
})

const secretsRoute = createRoute({
  getParentRoute: () => appRoute,
  path: "/secrets",
  component: SecretsPage,
})

function storedPrimaryRole(): string | null {
  return useAuthStore.getState().user?.primary_role ?? null
}

const rolesRoute = createRoute({
  getParentRoute: () => appRoute,
  path: "/roles",
  component: RolesPage,
  beforeLoad: () => {
    if (storedPrimaryRole() !== "admin") throw redirect({ to: "/app" })
  },
})

const tagsRoute = createRoute({
  getParentRoute: () => appRoute,
  path: "/tags",
  component: TagsPage,
  beforeLoad: () => {
    const role = storedPrimaryRole()
    if (role !== "admin" && role !== "contribute") {
      throw redirect({ to: "/app" })
    }
  },
})

const settingsRoute = createRoute({
  getParentRoute: () => appRoute,
  path: "/settings",
  beforeLoad: () => {
    // Settings lives in a sidebar modal now; keep the old URL as a soft redirect.
    throw redirect({ to: "/app" })
  },
})

const routeTree = rootRoute.addChildren([
  indexRoute,
  setupRoute,
  loginRoute,
  ssoCallbackRoute,
  pendingRoute,
  changePasswordRoute,
  appRoute.addChildren([
    overviewRoute,
    membersRoute,
    resourcesRoute,
    secretsRoute,
    rolesRoute,
    tagsRoute,
    settingsRoute,
  ]),
])

export const router = createRouter({ routeTree })

declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router
  }
}

function BootGate() {
  const hydrate = useAuthStore((s) => s.hydrate)
  const token = useAuthStore((s) => s.token)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    hydrate()
    void (async () => {
      try {
        const status = await api.setupStatus()
        if (!status.configured) {
          await router.navigate({ to: "/setup" })
          return
        }
        const t = authSession.getToken()
        if (t) {
          await router.navigate({ to: "/app" })
        } else {
          await router.navigate({ to: "/login" })
        }
      } catch (e) {
        setError(e instanceof Error ? e.message : "Backend unavailable")
      }
    })()
  }, [hydrate, token])

  return (
    <div className="flex min-h-screen items-center justify-center text-sm text-(--color-text-muted)">
      {error ?? "Starting Evo Conductor…"}
    </div>
  )
}
