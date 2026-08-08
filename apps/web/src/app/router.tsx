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
import { LoginPage } from "@/features/auth/pages/login-page"
import { SsoCallbackPage } from "@/features/auth/pages/sso-callback-page"
import { MembersPage } from "@/features/members/pages/members-page"
import { OverviewPage } from "@/features/dashboard/pages/overview-page"
import { ResourcesPage } from "@/features/resources/pages/resources-page"
import { RolesPage } from "@/features/roles/pages/roles-page"
import { SecretsPage } from "@/features/secrets/pages/secrets-page"
import { SetupPage } from "@/features/setup/pages/setup-page"
import { useAuthStore } from "@/shared/stores/auth"

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

function LoginRoute() {
  const { status } = loginRoute.useRouteContext() as { status: SetupStatus }
  return (
    <LoginPage
      projectName={status.project_name}
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
    const token = localStorage.getItem("conductor.token")
    if (!token) throw redirect({ to: "/login" })
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

const rolesRoute = createRoute({
  getParentRoute: () => appRoute,
  path: "/roles",
  component: RolesPage,
})

const routeTree = rootRoute.addChildren([
  indexRoute,
  setupRoute,
  loginRoute,
  ssoCallbackRoute,
  appRoute.addChildren([
    overviewRoute,
    membersRoute,
    resourcesRoute,
    secretsRoute,
    rolesRoute,
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
        const t = localStorage.getItem("conductor.token")
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
