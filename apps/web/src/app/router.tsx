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
import { SettingsPage } from "@/features/settings/pages/settings-page"
import { SetupPage } from "@/features/setup/pages/setup-page"
import { TagsPage } from "@/features/tags/pages/tags-page"
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
    const token = localStorage.getItem("conductor.token")
    if (!token) throw redirect({ to: "/login" })
  },
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

    try {
      const user = await api.me()
      localStorage.setItem("conductor.user", JSON.stringify(user))
      if (user.must_change_password) {
        throw redirect({ to: "/change-password" })
      }
    } catch (e) {
      if (e && typeof e === "object" && "to" in e) throw e
      localStorage.removeItem("conductor.token")
      localStorage.removeItem("conductor.user")
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
  const raw = localStorage.getItem("conductor.user")
  if (!raw) return null
  try {
    return (JSON.parse(raw) as { primary_role?: string }).primary_role ?? null
  } catch {
    return null
  }
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
  component: SettingsPage,
  beforeLoad: () => {
    if (storedPrimaryRole() !== "admin") throw redirect({ to: "/app" })
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
