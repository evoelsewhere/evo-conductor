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
import { MemberActivityPage } from "@/features/members/pages/member-activity-page"
import { MemberDetailPage } from "@/features/members/pages/member-detail-page"
import { MemberRequestDetailPage } from "@/features/members/pages/member-request-detail-page"
import { MemberToolsPage } from "@/features/members/pages/member-tools-page"
import { OverviewPage } from "@/features/dashboard/pages/overview-page"
import { ResourcesPage } from "@/features/resources/pages/resources-page"
import { ResourceGovernancePage } from "@/features/resources/pages/resource-governance-page"
import { ResourceStudioPage } from "@/features/resources/pages/resource-studio-page"
import { ResourceUsagePage } from "@/features/resource-usage/pages/resource-usage-page"
import { ResourceRequestDetailPage } from "@/features/resource-usage/pages/resource-request-detail-page"
import { RolesPage } from "@/features/roles/pages/roles-page"
import { SecretsPage } from "@/features/secrets/pages/secrets-page"
import { SetupPage } from "@/features/setup/pages/setup-page"
import { TagsPage } from "@/features/tags/pages/tags-page"
import { useAuthStore } from "@/shared/stores/auth"
import { authSession } from "@/shared/lib/auth-session"
import { PRIMARY_ROLE, type PrimaryRole } from "@/shared/constants/member"
import {
  RESOURCE_USAGE_ROUTE_PATHS,
  RESOURCE_USAGE_VIEW,
} from "@/shared/constants/resource-usage"
import { RESOURCE_KIND } from "@/shared/constants/resource"
import { RESOURCE_KIND_USAGE_ROUTE_PATHS } from "@/shared/constants/resource-monitoring"

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
    if (role !== PRIMARY_ROLE.ADMIN && role !== PRIMARY_ROLE.CONTRIBUTE) {
      throw redirect({ to: "/app" })
    }
  },
})

function requireTelemetryAccess() {
  const role = storedPrimaryRole()
  if (role !== PRIMARY_ROLE.ADMIN && role !== PRIMARY_ROLE.CONTRIBUTE) {
    throw redirect({ to: "/app" })
  }
}

const memberDetailRoute = createRoute({
  getParentRoute: () => appRoute,
  path: "/members/$userId",
  component: MemberDetailPage,
  beforeLoad: requireTelemetryAccess,
})

const memberActivityRoute = createRoute({
  getParentRoute: () => appRoute,
  path: "/members/$userId/activity",
  component: MemberActivityPage,
  beforeLoad: requireTelemetryAccess,
})

const memberRequestDetailRoute = createRoute({
  getParentRoute: () => appRoute,
  path: "/members/$userId/activity/$requestId",
  component: MemberRequestDetailPage,
  beforeLoad: requireTelemetryAccess,
})

const memberToolsRoute = createRoute({
  getParentRoute: () => appRoute,
  path: "/members/$userId/tools",
  component: MemberToolsPage,
  beforeLoad: requireTelemetryAccess,
})

const resourcesRoute = createRoute({
  getParentRoute: () => appRoute,
  path: "/resources",
  component: ResourcesPage,
})

const resourceUsageRoute = createRoute({
  getParentRoute: () => appRoute,
  path: RESOURCE_USAGE_ROUTE_PATHS.overview,
  component: () => <ResourceUsagePage view={RESOURCE_USAGE_VIEW.OVERVIEW} />,
  beforeLoad: requireTelemetryAccess,
})

const resourceUsageActivityRoute = createRoute({
  getParentRoute: () => appRoute,
  path: RESOURCE_USAGE_ROUTE_PATHS.activity,
  component: () => <ResourceUsagePage view={RESOURCE_USAGE_VIEW.ACTIVITY} />,
  beforeLoad: requireTelemetryAccess,
})

const resourceUsageAnalysisRoute = createRoute({
  getParentRoute: () => appRoute,
  path: RESOURCE_USAGE_ROUTE_PATHS.usage,
  component: () => <ResourceUsagePage view={RESOURCE_USAGE_VIEW.USAGE} />,
  beforeLoad: requireTelemetryAccess,
})

const resourceRequestDetailRoute = createRoute({
  getParentRoute: () => appRoute,
  path: RESOURCE_USAGE_ROUTE_PATHS.requestDetail,
  component: ResourceRequestDetailPage,
  beforeLoad: requireTelemetryAccess,
})

const legacyResourceUsageRoute = createRoute({
  getParentRoute: () => appRoute,
  path: RESOURCE_USAGE_ROUTE_PATHS.legacy,
  beforeLoad: () => {
    requireTelemetryAccess()
    throw redirect({ to: "/app/resources/usage" })
  },
})

const resourcesPluginsRoute = createRoute({
  getParentRoute: () => appRoute,
  path: "/resources/plugins",
  component: () => <ResourcesPage fixedKind="plugin" />,
})

const resourcesPluginsActivityRoute = createRoute({
  getParentRoute: () => appRoute,
  path: RESOURCE_KIND_USAGE_ROUTE_PATHS.plugin.activity,
  component: () => <ResourceUsagePage view={RESOURCE_USAGE_VIEW.ACTIVITY} scopeKind={RESOURCE_KIND.PLUGIN} />,
  beforeLoad: requireTelemetryAccess,
})

const resourcesPluginsUsageRoute = createRoute({
  getParentRoute: () => appRoute,
  path: RESOURCE_KIND_USAGE_ROUTE_PATHS.plugin.usage,
  component: () => <ResourceUsagePage view={RESOURCE_USAGE_VIEW.USAGE} scopeKind={RESOURCE_KIND.PLUGIN} />,
  beforeLoad: requireTelemetryAccess,
})

const resourceGovernanceRoute = createRoute({
  getParentRoute: () => appRoute,
  path: "/resources/$kind/$resourceId",
  component: () => <ResourceGovernancePage view="overview" />,
})

const resourceGovernanceAccessRoute = createRoute({
  getParentRoute: () => appRoute,
  path: "/resources/$kind/$resourceId/access",
  component: () => <ResourceGovernancePage view="access" />,
})

const resourceGovernanceFeedbackRoute = createRoute({
  getParentRoute: () => appRoute,
  path: "/resources/$kind/$resourceId/feedback",
  component: () => <ResourceGovernancePage view="feedback" />,
})

const resourceStudioRoute = createRoute({
  getParentRoute: () => appRoute,
  path: "/resources/$kind/$resourceId/edit",
  component: ResourceStudioPage,
  beforeLoad: async ({ params }) => {
    const role = storedPrimaryRole()
    if (role !== PRIMARY_ROLE.ADMIN && role !== PRIMARY_ROLE.CONTRIBUTE) {
      throw redirect({
        to: "/app/resources/$kind/$resourceId",
        params,
      })
    }
    if (role === PRIMARY_ROLE.CONTRIBUTE) {
      const actor = useAuthStore.getState().user
      const resource = (await api.resources()).find(
        (item) => item.id === params.resourceId && item.kind === params.kind,
      )
      if (!actor || resource?.owner_user_id !== actor.id) {
        throw redirect({
          to: "/app/resources/$kind/$resourceId",
          params,
        })
      }
    }
  },
})

const resourcesSkillsRoute = createRoute({
  getParentRoute: () => appRoute,
  path: "/resources/skills",
  component: () => <ResourcesPage fixedKind="skill" />,
})

const resourcesSkillsActivityRoute = createRoute({
  getParentRoute: () => appRoute,
  path: RESOURCE_KIND_USAGE_ROUTE_PATHS.skill.activity,
  component: () => <ResourceUsagePage view={RESOURCE_USAGE_VIEW.ACTIVITY} scopeKind={RESOURCE_KIND.SKILL} />,
  beforeLoad: requireTelemetryAccess,
})

const resourcesSkillsUsageRoute = createRoute({
  getParentRoute: () => appRoute,
  path: RESOURCE_KIND_USAGE_ROUTE_PATHS.skill.usage,
  component: () => <ResourceUsagePage view={RESOURCE_USAGE_VIEW.USAGE} scopeKind={RESOURCE_KIND.SKILL} />,
  beforeLoad: requireTelemetryAccess,
})

const resourcesAgentsRoute = createRoute({
  getParentRoute: () => appRoute,
  path: "/resources/agents",
  component: () => <ResourcesPage fixedKind="agent" />,
})

const resourcesAgentsActivityRoute = createRoute({
  getParentRoute: () => appRoute,
  path: RESOURCE_KIND_USAGE_ROUTE_PATHS.agent.activity,
  component: () => <ResourceUsagePage view={RESOURCE_USAGE_VIEW.ACTIVITY} scopeKind={RESOURCE_KIND.AGENT} />,
  beforeLoad: requireTelemetryAccess,
})

const resourcesAgentsUsageRoute = createRoute({
  getParentRoute: () => appRoute,
  path: RESOURCE_KIND_USAGE_ROUTE_PATHS.agent.usage,
  component: () => <ResourceUsagePage view={RESOURCE_USAGE_VIEW.USAGE} scopeKind={RESOURCE_KIND.AGENT} />,
  beforeLoad: requireTelemetryAccess,
})

const secretsRoute = createRoute({
  getParentRoute: () => appRoute,
  path: "/secrets",
  component: SecretsPage,
})

function storedPrimaryRole(): PrimaryRole | null {
  return useAuthStore.getState().user?.primary_role ?? null
}

const rolesRoute = createRoute({
  getParentRoute: () => appRoute,
  path: "/roles",
  component: RolesPage,
  beforeLoad: () => {
    if (storedPrimaryRole() !== PRIMARY_ROLE.ADMIN) throw redirect({ to: "/app" })
  },
})

const tagsRoute = createRoute({
  getParentRoute: () => appRoute,
  path: "/tags",
  component: TagsPage,
  beforeLoad: () => {
    const role = storedPrimaryRole()
    if (role !== PRIMARY_ROLE.ADMIN && role !== PRIMARY_ROLE.CONTRIBUTE) {
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
    memberDetailRoute,
    memberActivityRoute,
    memberRequestDetailRoute,
    memberToolsRoute,
    resourcesRoute,
    resourceUsageRoute,
    resourceUsageActivityRoute,
    resourceUsageAnalysisRoute,
    resourceRequestDetailRoute,
    legacyResourceUsageRoute,
    resourcesPluginsRoute,
    resourcesPluginsActivityRoute,
    resourcesPluginsUsageRoute,
    resourcesSkillsRoute,
    resourcesSkillsActivityRoute,
    resourcesSkillsUsageRoute,
    resourcesAgentsRoute,
    resourcesAgentsActivityRoute,
    resourcesAgentsUsageRoute,
    resourceGovernanceRoute,
    resourceGovernanceAccessRoute,
    resourceGovernanceFeedbackRoute,
    resourceStudioRoute,
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
