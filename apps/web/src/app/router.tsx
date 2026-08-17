import {
  createRootRoute,
  createRoute,
  createRouter,
  redirect,
  Outlet,
  useParams,
} from "@tanstack/react-router"
import { useEffect, useState, type ReactNode } from "react"

import {
  api,
  ApiError,
  type PermissionKey,
  type SetupStatus,
} from "@/shared/api/client"
import { AppShell } from "@/shared/components/app-shell"
import { PageFrame } from "@/shared/components/page-frame"
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
import {
  AUTHORIZATION_DECISION,
  PERMISSION,
  bestAuthorizationDecision,
  mayRequest,
  type AuthorizationTargetContext,
} from "@/shared/lib/authorization"
import { ErrorState } from "@/shared/ui/empty-state"
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
  beforeLoad: async ({ location }) => {
    const status = await api.setupStatus()
    if (!status.configured) throw redirect({ to: "/setup" })
    const token = authSession.getToken()
    if (!token) throw redirect({ to: "/login" })

    try {
      await useAuthStore.getState().refreshAuthorization()
      const user = useAuthStore.getState().user
      if (!user) throw new Error("The current session could not be loaded.")
      if (user.must_change_password) {
        throw redirect({ to: "/change-password" })
      }
      const authorizationState = useAuthStore.getState()
      if (
        location.pathname.replace(/\/+$/, "") === "/app" &&
        authorizationState.can(PERMISSION.PROJECT_DASHBOARD_READ) ===
          AUTHORIZATION_DECISION.DENY &&
        mayRequest(authorizationState.can(PERMISSION.RESOURCE_CONSUME))
      ) {
        throw redirect({ to: "/app/resources" })
      }
    } catch (e) {
      if (e && typeof e === "object" && "to" in e) throw e
      if (e instanceof ApiError && e.status === 401) {
        useAuthStore.getState().clear()
        return
      }
    }
  },
})

function PermissionBoundary({
  permissions,
  target,
  children,
}: {
  permissions: PermissionKey[]
  target?: Omit<AuthorizationTargetContext, "actorId">
  children: ReactNode
}) {
  const can = useAuthStore((state) => state.can)
  const decision = bestAuthorizationDecision(
    permissions.map((permission) => can(permission, target)),
  )
  if (decision === AUTHORIZATION_DECISION.DENY) {
    return (
      <PageFrame
        title="Access unavailable"
        subtitle="Your current project permissions do not include this screen."
      >
        <ErrorState message="Forbidden. Ask a project administrator if you need this access." />
      </PageFrame>
    )
  }
  return children
}

function OverviewRoutePage() {
  return (
    <PermissionBoundary permissions={[PERMISSION.PROJECT_DASHBOARD_READ]}>
      <OverviewPage />
    </PermissionBoundary>
  )
}

function MembersRoutePage() {
  return (
    <PermissionBoundary permissions={[PERMISSION.MEMBER_DIRECTORY_READ]}>
      <MembersPage />
    </PermissionBoundary>
  )
}

function MemberPrivateBoundary({ children }: { children: ReactNode }) {
  const { userId } = useParamsForPermission()
  return (
    <PermissionBoundary
      permissions={[
        PERMISSION.MEMBER_PRIVATE_READ_SELF,
        PERMISSION.MEMBER_PRIVATE_READ_ANY,
      ]}
      target={{ targetId: userId }}
    >
      {children}
    </PermissionBoundary>
  )
}

function MemberTelemetryBoundary({ children }: { children: ReactNode }) {
  const { userId } = useParamsForPermission()
  return (
    <PermissionBoundary
      permissions={[
        PERMISSION.TELEMETRY_MEMBER_READ_SELF,
        PERMISSION.TELEMETRY_MEMBER_READ_ANY,
      ]}
      target={{ targetId: userId }}
    >
      {children}
    </PermissionBoundary>
  )
}

function useParamsForPermission(): { userId: string } {
  const { userId } = useParams({ strict: false }) as { userId?: string }
  return { userId: userId ?? "" }
}

const overviewRoute = createRoute({
  getParentRoute: () => appRoute,
  path: "/",
  component: OverviewRoutePage,
})

const membersRoute = createRoute({
  getParentRoute: () => appRoute,
  path: "/members",
  component: MembersRoutePage,
})

const memberDetailRoute = createRoute({
  getParentRoute: () => appRoute,
  path: "/members/$userId",
  component: () => <MemberPrivateBoundary><MemberDetailPage /></MemberPrivateBoundary>,
})

const memberActivityRoute = createRoute({
  getParentRoute: () => appRoute,
  path: "/members/$userId/activity",
  component: () => <MemberTelemetryBoundary><MemberActivityPage /></MemberTelemetryBoundary>,
})

const memberRequestDetailRoute = createRoute({
  getParentRoute: () => appRoute,
  path: "/members/$userId/activity/$requestId",
  component: () => <MemberTelemetryBoundary><MemberRequestDetailPage /></MemberTelemetryBoundary>,
})

const memberToolsRoute = createRoute({
  getParentRoute: () => appRoute,
  path: "/members/$userId/tools",
  component: () => <MemberTelemetryBoundary><MemberToolsPage /></MemberTelemetryBoundary>,
})

const resourcesRoute = createRoute({
  getParentRoute: () => appRoute,
  path: "/resources",
  component: () => <PermissionBoundary permissions={[PERMISSION.RESOURCE_CONSUME]}><ResourcesPage /></PermissionBoundary>,
})

const resourceUsageRoute = createRoute({
  getParentRoute: () => appRoute,
  path: RESOURCE_USAGE_ROUTE_PATHS.overview,
  component: () => <PermissionBoundary permissions={[PERMISSION.TELEMETRY_PROJECT_READ]}><ResourceUsagePage view={RESOURCE_USAGE_VIEW.OVERVIEW} /></PermissionBoundary>,
})

const resourceUsageActivityRoute = createRoute({
  getParentRoute: () => appRoute,
  path: RESOURCE_USAGE_ROUTE_PATHS.activity,
  component: () => <PermissionBoundary permissions={[PERMISSION.TELEMETRY_MEMBER_READ_ANY]}><ResourceUsagePage view={RESOURCE_USAGE_VIEW.ACTIVITY} /></PermissionBoundary>,
})

const resourceUsageAnalysisRoute = createRoute({
  getParentRoute: () => appRoute,
  path: RESOURCE_USAGE_ROUTE_PATHS.usage,
  component: () => <PermissionBoundary permissions={[PERMISSION.TELEMETRY_PROJECT_READ]}><ResourceUsagePage view={RESOURCE_USAGE_VIEW.USAGE} /></PermissionBoundary>,
})

const resourceRequestDetailRoute = createRoute({
  getParentRoute: () => appRoute,
  path: RESOURCE_USAGE_ROUTE_PATHS.requestDetail,
  component: () => <PermissionBoundary permissions={[PERMISSION.TELEMETRY_MEMBER_READ_ANY]}><ResourceRequestDetailPage /></PermissionBoundary>,
})

const legacyResourceUsageRoute = createRoute({
  getParentRoute: () => appRoute,
  path: RESOURCE_USAGE_ROUTE_PATHS.legacy,
  beforeLoad: () => {
    throw redirect({ to: "/app/resources/usage" })
  },
})

const resourcesPluginsRoute = createRoute({
  getParentRoute: () => appRoute,
  path: "/resources/plugins",
  component: () => <PermissionBoundary permissions={[PERMISSION.RESOURCE_CONSUME]}><ResourcesPage fixedKind="plugin" /></PermissionBoundary>,
})

const resourcesPluginsActivityRoute = createRoute({
  getParentRoute: () => appRoute,
  path: RESOURCE_KIND_USAGE_ROUTE_PATHS.plugin.activity,
  component: () => <PermissionBoundary permissions={[PERMISSION.TELEMETRY_MEMBER_READ_ANY]}><ResourceUsagePage view={RESOURCE_USAGE_VIEW.ACTIVITY} scopeKind={RESOURCE_KIND.PLUGIN} /></PermissionBoundary>,
})

const resourcesPluginsUsageRoute = createRoute({
  getParentRoute: () => appRoute,
  path: RESOURCE_KIND_USAGE_ROUTE_PATHS.plugin.usage,
  component: () => <PermissionBoundary permissions={[PERMISSION.TELEMETRY_PROJECT_READ]}><ResourceUsagePage view={RESOURCE_USAGE_VIEW.USAGE} scopeKind={RESOURCE_KIND.PLUGIN} /></PermissionBoundary>,
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
})

const resourcesSkillsRoute = createRoute({
  getParentRoute: () => appRoute,
  path: "/resources/skills",
  component: () => <PermissionBoundary permissions={[PERMISSION.RESOURCE_CONSUME]}><ResourcesPage fixedKind="skill" /></PermissionBoundary>,
})

const resourcesSkillsActivityRoute = createRoute({
  getParentRoute: () => appRoute,
  path: RESOURCE_KIND_USAGE_ROUTE_PATHS.skill.activity,
  component: () => <PermissionBoundary permissions={[PERMISSION.TELEMETRY_MEMBER_READ_ANY]}><ResourceUsagePage view={RESOURCE_USAGE_VIEW.ACTIVITY} scopeKind={RESOURCE_KIND.SKILL} /></PermissionBoundary>,
})

const resourcesSkillsUsageRoute = createRoute({
  getParentRoute: () => appRoute,
  path: RESOURCE_KIND_USAGE_ROUTE_PATHS.skill.usage,
  component: () => <PermissionBoundary permissions={[PERMISSION.TELEMETRY_PROJECT_READ]}><ResourceUsagePage view={RESOURCE_USAGE_VIEW.USAGE} scopeKind={RESOURCE_KIND.SKILL} /></PermissionBoundary>,
})

const resourcesAgentsRoute = createRoute({
  getParentRoute: () => appRoute,
  path: "/resources/agents",
  component: () => <PermissionBoundary permissions={[PERMISSION.RESOURCE_CONSUME]}><ResourcesPage fixedKind="agent" /></PermissionBoundary>,
})

const resourcesAgentsActivityRoute = createRoute({
  getParentRoute: () => appRoute,
  path: RESOURCE_KIND_USAGE_ROUTE_PATHS.agent.activity,
  component: () => <PermissionBoundary permissions={[PERMISSION.TELEMETRY_MEMBER_READ_ANY]}><ResourceUsagePage view={RESOURCE_USAGE_VIEW.ACTIVITY} scopeKind={RESOURCE_KIND.AGENT} /></PermissionBoundary>,
})

const resourcesAgentsUsageRoute = createRoute({
  getParentRoute: () => appRoute,
  path: RESOURCE_KIND_USAGE_ROUTE_PATHS.agent.usage,
  component: () => <PermissionBoundary permissions={[PERMISSION.TELEMETRY_PROJECT_READ]}><ResourceUsagePage view={RESOURCE_USAGE_VIEW.USAGE} scopeKind={RESOURCE_KIND.AGENT} /></PermissionBoundary>,
})

const secretsRoute = createRoute({
  getParentRoute: () => appRoute,
  path: "/secrets",
  component: () => {
    const userId = useAuthStore((state) => state.user?.id)
    return <PermissionBoundary permissions={[PERMISSION.CONNECTION_TOKEN_READ_SELF]} target={{ ownerId: userId }}><SecretsPage /></PermissionBoundary>
  },
})

const rolesRoute = createRoute({
  getParentRoute: () => appRoute,
  path: "/roles",
  component: () => <PermissionBoundary permissions={[PERMISSION.TAXONOMY_READ]}><RolesPage /></PermissionBoundary>,
})

const tagsRoute = createRoute({
  getParentRoute: () => appRoute,
  path: "/tags",
  component: () => <PermissionBoundary permissions={[PERMISSION.TAXONOMY_READ]}><TagsPage /></PermissionBoundary>,
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
