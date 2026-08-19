import type { Page, Route } from "@playwright/test"

export type DashboardRole = "admin" | "contribute" | "user"

export interface DashboardRequestLog {
  analytics: string[]
  dashboard: string[]
  pendingCount: number
  unexpected: string[]
}

export interface DashboardScenarioOptions {
  summary?: Record<string, unknown>
  analytics?: Record<string, unknown>
  governedAnalytics?: Record<string, unknown>
  summaryGate?: Promise<void>
  analyticsGate?: Promise<void>
  summaryFailure?: DashboardScenarioFailure
  analyticsFailure?: DashboardScenarioFailure
}

interface DashboardScenarioFailure {
  status: number
  message: string
}

const PERMISSIONS = [
  "authorization.grants.read_self",
  "session.self.read",
  "session.password.change",
  "project.branding.read",
  "project.dashboard.read",
  "project.settings.read",
  "project.settings.manage",
  "member.directory.read",
  "member.manage",
  "member.private.read_self",
  "member.private.read_any",
  "telemetry.project.read",
  "telemetry.member.read_self",
  "telemetry.member.read_any",
  "taxonomy.read",
  "taxonomy.definition.manage",
  "member.tag_assignment.manage",
  "resource.consume",
  "resource.author",
  "resource.access.manage",
  "resource.lifecycle.manage",
  "resource.release.non_executable",
  "resource.release.restricted",
  "resource.monitoring.aggregate.read",
  "resource.monitoring.member_detail.read",
  "resource.feedback.submit",
  "resource.feedback.read",
  "analytics_view.read",
  "analytics_view.manage_self",
  "analytics_view.manage_any",
  "connection_token.issue_self",
  "connection_token.read_self",
  "connection_token.revoke_self",
  "connection_token.read_any",
  "connection_token.revoke_any",
  "audit.read",
  "audit.export",
] as const

const CONDITION_METADATA = [
  { kind: "any", evaluation: "ui_target_context" },
  { kind: "self", evaluation: "ui_target_context" },
  { kind: "owner", evaluation: "ui_target_context" },
  { kind: "resource_kind_in", evaluation: "ui_target_context" },
  { kind: "lifecycle_in", evaluation: "ui_target_context" },
  { kind: "same_project", evaluation: "server_only" },
  { kind: "effective_audience", evaluation: "server_only" },
] as const

const ROLE_PERMISSIONS: Record<DashboardRole, readonly (typeof PERMISSIONS)[number][]> = {
  admin: PERMISSIONS,
  contribute: [
    "authorization.grants.read_self",
    "session.self.read",
    "session.password.change",
    "project.branding.read",
    "project.dashboard.read",
    "member.directory.read",
    "telemetry.project.read",
    "taxonomy.read",
    "resource.consume",
    "resource.author",
    "resource.monitoring.aggregate.read",
    "analytics_view.read",
    "analytics_view.manage_self",
    "connection_token.issue_self",
    "connection_token.read_self",
    "connection_token.revoke_self",
  ],
  user: [
    "authorization.grants.read_self",
    "session.self.read",
    "session.password.change",
    "project.branding.read",
    "resource.consume",
    "resource.feedback.submit",
    "connection_token.issue_self",
    "connection_token.read_self",
    "connection_token.revoke_self",
  ],
}

export const DASHBOARD_MEMBER_ID = "11111111-1111-4111-8111-111111111111"
export const DASHBOARD_MEMBER_NAME = "Lan Nguyen"
export const DASHBOARD_MEMBER_EMAIL = "lan.nguyen@example.test"

const USERS: Record<DashboardRole, Record<string, unknown>> = {
  admin: user("aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa", "admin"),
  contribute: user("bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb", "contribute"),
  user: user("cccccccc-cccc-4ccc-8ccc-cccccccccccc", "user"),
}

export const POPULATED_DASHBOARD_SUMMARY = {
  project_name: "Playwright Mission Control",
  members_total: 28,
  members_online: 7,
  secrets_active: 16,
  resources: {
    agents: 8,
    skills: 12,
    plugins: 5,
    workflows: 2,
  },
  sso_enabled: true,
  presence: {
    clients_seen_recently: 9,
    members_seen_recently: 7,
    threshold_seconds: 180,
    observed_at: "2026-08-18T01:30:00Z",
  },
  realtime: {
    active_owners: 7,
    active_streams: 11,
    scope: "this_node",
  },
  host_metrics: {
    cpu_usage_percent: 37.4,
    memory_used_bytes: 8 * 1024 * 1024 * 1024,
    memory_total_bytes: 16 * 1024 * 1024 * 1024,
    gpu_usage_percent: null,
    vram_used_bytes: null,
    vram_total_bytes: null,
    sampled_at: "2026-08-18T01:30:00Z",
    scope: "conductor_host",
  },
  feedback: {
    scope: "project",
    count: 45,
    average_rating: 4.4,
    positive_count: 39,
    positive_percent: 86.7,
    distribution: {
      rating_1: 1,
      rating_2: 2,
      rating_3: 3,
      rating_4: 12,
      rating_5: 27,
    },
  },
}

export const UNSUPPORTED_RUNTIME_DASHBOARD_SUMMARY = {
  ...POPULATED_DASHBOARD_SUMMARY,
  host_metrics: {
    ...POPULATED_DASHBOARD_SUMMARY.host_metrics,
    cpu_usage_percent: null,
    memory_used_bytes: null,
    memory_total_bytes: null,
    gpu_usage_percent: null,
    vram_used_bytes: null,
    vram_total_bytes: null,
  },
}

export const CONTRIBUTOR_DASHBOARD_SUMMARY = {
  ...POPULATED_DASHBOARD_SUMMARY,
  feedback: {
    ...POPULATED_DASHBOARD_SUMMARY.feedback,
    scope: "owned_resources",
    count: 12,
    positive_count: 10,
    positive_percent: 83.3,
    distribution: {
      rating_1: 0,
      rating_2: 1,
      rating_3: 1,
      rating_4: 4,
      rating_5: 6,
    },
  },
}

export const GOVERNED_DASHBOARD_ANALYTICS = {
  from: "2026-07-19T01:30:00Z",
  to: "2026-08-18T01:30:00Z",
  scope: "governed",
  totals: {
    reported_installations: 12,
    installed_installations: 9,
    installed_members: 7,
    pending_installations: 2,
    attention_installations: 1,
    all_requests: 1_600,
    governed_requests: 1_200,
    requests: 1_200,
    resource_uses: 1_750,
    model_calls: 1_600,
    tool_calls: 900,
    successes: 960,
    errors: 120,
    blocked: 90,
    cancelled: 30,
    tokens_in: 4_800_000,
    tokens_out: 1_200_000,
    cache_read_tokens: 600_000,
    reasoning_tokens: 300_000,
    tool_use_tokens: 100_000,
    total_tokens: 6_000_000,
    estimated_cost_usd_micros: 12_500_000,
    unpriced_model_calls: 14,
    average_tokens_per_request: 5_000,
    average_duration_ms: 1_450,
  },
  daily: [
    usageDay("2026-08-16", 360, 288, 36, 27, 9, 1_900_000, 3_200_000),
    usageDay("2026-08-17", 410, 328, 41, 31, 10, 2_350_000, 4_100_000),
    usageDay("2026-08-18", 430, 344, 43, 32, 11, 2_750_000, 5_200_000),
  ],
  resources: [
    {
      resource_id: "21111111-1111-4111-8111-111111111111",
      version_id: "31111111-1111-4111-8111-111111111111",
      kind: "agent",
      name: "Research copilot",
      version: "4.2.0",
      relation: "executing_agent",
      uses: 620,
      members: 7,
      requests: 510,
      successes: 450,
      errors: 35,
      model_calls: 730,
      tool_calls: 410,
      total_tokens: 3_100_000,
      estimated_cost_usd_micros: 5_600_000,
      last_used_at: "2026-08-18T01:25:00Z",
    },
    {
      resource_id: "22222222-2222-4222-8222-222222222222",
      version_id: "32222222-2222-4222-8222-222222222222",
      kind: "skill",
      name: "Release review",
      version: "2.1.0",
      relation: "activated_skill",
      uses: 480,
      members: 6,
      requests: 420,
      successes: 350,
      errors: 40,
      model_calls: 490,
      tool_calls: 210,
      total_tokens: 2_100_000,
      estimated_cost_usd_micros: 3_900_000,
      last_used_at: "2026-08-18T01:20:00Z",
    },
    {
      resource_id: "23333333-3333-4333-8333-333333333333",
      version_id: "33333333-3333-4333-8333-333333333333",
      kind: "plugin",
      name: "Browser tools",
      version: "3.0.1",
      relation: "plugin_contributed_tool",
      uses: 350,
      members: 5,
      requests: 270,
      successes: 220,
      errors: 30,
      model_calls: 380,
      tool_calls: 280,
      total_tokens: 1_800_000,
      estimated_cost_usd_micros: 3_000_000,
      last_used_at: "2026-08-18T01:15:00Z",
    },
  ],
  members: [
    {
      user_id: DASHBOARD_MEMBER_ID,
      display_name: DASHBOARD_MEMBER_NAME,
      email: DASHBOARD_MEMBER_EMAIL,
      primary_role: "contribute",
      requests: 420,
      resource_uses: 610,
      model_calls: 560,
      tool_calls: 315,
      installations: 2,
      total_tokens: 2_500_000,
      estimated_cost_usd_micros: 4_700_000,
      last_received_at: "2026-08-18T01:26:00Z",
    },
    {
      user_id: "12222222-2222-4222-8222-222222222222",
      display_name: "Minh Tran",
      email: "minh.tran@example.test",
      primary_role: "user",
      requests: 330,
      resource_uses: 480,
      model_calls: 430,
      tool_calls: 260,
      installations: 1,
      total_tokens: 1_900_000,
      estimated_cost_usd_micros: 3_200_000,
      last_received_at: "2026-08-18T01:19:00Z",
    },
    {
      user_id: "13333333-3333-4333-8333-333333333333",
      display_name: "Ha Anh",
      email: "ha.anh@example.test",
      primary_role: "admin",
      requests: 250,
      resource_uses: 370,
      model_calls: 310,
      tool_calls: 190,
      installations: 1,
      total_tokens: 1_500_000,
      estimated_cost_usd_micros: 2_700_000,
      last_received_at: "2026-08-18T00:58:00Z",
    },
  ],
  models: [
    {
      provider: "openai",
      model: "gpt-5",
      calls: 780,
      total_tokens: 3_100_000,
      estimated_cost_usd_micros: 6_100_000,
      unpriced_calls: 0,
    },
    {
      provider: "anthropic",
      model: "claude-sonnet-4",
      calls: 530,
      total_tokens: 2_000_000,
      estimated_cost_usd_micros: 4_800_000,
      unpriced_calls: 0,
    },
    {
      provider: "custom",
      model: "local-reasoner",
      calls: 290,
      total_tokens: 900_000,
      estimated_cost_usd_micros: 1_600_000,
      unpriced_calls: 14,
    },
  ],
  roles: [
    {
      primary_role: "contribute",
      requests: 600,
      model_calls: 780,
      tool_calls: 470,
      total_tokens: 3_000_000,
      estimated_cost_usd_micros: 6_300_000,
    },
    {
      primary_role: "user",
      requests: 400,
      model_calls: 540,
      tool_calls: 310,
      total_tokens: 2_000_000,
      estimated_cost_usd_micros: 3_900_000,
    },
    {
      primary_role: "admin",
      requests: 200,
      model_calls: 280,
      tool_calls: 120,
      total_tokens: 1_000_000,
      estimated_cost_usd_micros: 2_300_000,
    },
  ],
  tools: [
    tool("web.search", "web", 360, 350, 7, 2, 1, 820),
    tool("filesystem.read", "filesystem", 310, 306, 3, 1, 0, 45),
    tool("git.diff", "version_control", 230, 220, 8, 2, 0, 125),
  ],
  activity: [
    {
      request_id: "request-dashboard-1",
      user_id: DASHBOARD_MEMBER_ID,
      display_name: DASHBOARD_MEMBER_NAME,
      primary_role: "contribute",
      resource_id: "21111111-1111-4111-8111-111111111111",
      version_id: "31111111-1111-4111-8111-111111111111",
      kind: "agent",
      resource_name: "Research copilot",
      version: "4.2.0",
      relation: "executing_agent",
      occurred_at: "2026-08-18T01:25:00Z",
      status: "success",
      provider: "openai",
      model: "gpt-5",
      model_calls: 2,
      tool_calls: 1,
      total_tokens: 5_400,
      estimated_cost_usd_micros: 9_500,
      unpriced_model_calls: 0,
      duration_ms: 1_320,
    },
  ],
  activity_total: 1,
  limit: 3,
  offset: 0,
}

export const POPULATED_DASHBOARD_ANALYTICS = {
  ...GOVERNED_DASHBOARD_ANALYTICS,
  scope: "all",
  totals: {
    ...GOVERNED_DASHBOARD_ANALYTICS.totals,
    requests: 1_600,
    model_calls: 2_000,
    tool_calls: 1_300,
    successes: 1_280,
    errors: 160,
    blocked: 120,
    cancelled: 40,
    tokens_in: 6_400_000,
    tokens_out: 1_600_000,
    total_tokens: 8_000_000,
    estimated_cost_usd_micros: 16_000_000,
    unpriced_model_calls: 20,
    average_tokens_per_request: 5_000,
  },
  daily: [
    usageDay("2026-08-16", 480, 384, 48, 36, 12, 2_400_000, 4_200_000),
    usageDay("2026-08-17", 540, 432, 54, 40, 14, 2_700_000, 5_300_000),
    usageDay("2026-08-18", 580, 464, 58, 44, 14, 2_900_000, 6_500_000),
  ],
  resources: [],
  members: GOVERNED_DASHBOARD_ANALYTICS.members.map((member, index) => ({
    ...member,
    requests: member.requests + [160, 140, 100][index],
    model_calls: member.model_calls + [210, 120, 70][index],
    tool_calls: member.tool_calls + [150, 110, 80][index],
    total_tokens: member.total_tokens + [900_000, 650_000, 450_000][index],
    estimated_cost_usd_micros:
      member.estimated_cost_usd_micros + [1_700_000, 1_100_000, 700_000][index],
  })),
  models: [
    { ...GOVERNED_DASHBOARD_ANALYTICS.models[0], calls: 1_000, total_tokens: 4_100_000, estimated_cost_usd_micros: 7_800_000 },
    { ...GOVERNED_DASHBOARD_ANALYTICS.models[1], calls: 650, total_tokens: 2_700_000, estimated_cost_usd_micros: 5_900_000 },
    { ...GOVERNED_DASHBOARD_ANALYTICS.models[2], calls: 350, total_tokens: 1_200_000, estimated_cost_usd_micros: 2_300_000, unpriced_calls: 20 },
  ],
  roles: [
    { ...GOVERNED_DASHBOARD_ANALYTICS.roles[0], requests: 800, model_calls: 1_000, tool_calls: 650, total_tokens: 4_000_000, estimated_cost_usd_micros: 8_000_000 },
    { ...GOVERNED_DASHBOARD_ANALYTICS.roles[1], requests: 500, model_calls: 650, tool_calls: 400, total_tokens: 2_500_000, estimated_cost_usd_micros: 5_000_000 },
    { ...GOVERNED_DASHBOARD_ANALYTICS.roles[2], requests: 300, model_calls: 350, tool_calls: 250, total_tokens: 1_500_000, estimated_cost_usd_micros: 3_000_000 },
  ],
  tools: [
    tool("web.search", "web", 500, 485, 10, 4, 1, 820),
    tool("filesystem.read", "filesystem", 430, 422, 5, 3, 0, 45),
    tool("git.diff", "version_control", 370, 354, 11, 5, 0, 125),
  ],
  activity: [],
  activity_total: 0,
}

export async function installDashboardScenario(
  page: Page,
  role: DashboardRole,
  options: DashboardScenarioOptions = {},
): Promise<DashboardRequestLog> {
  const currentUser = USERS[role]
  const log: DashboardRequestLog = {
    analytics: [],
    dashboard: [],
    pendingCount: 0,
    unexpected: [],
  }

  await page.addInitScript(
    ({ userValue }) => {
      window.localStorage.setItem("conductor.theme", "light")
      window.sessionStorage.setItem("conductor.token", "playwright-dashboard-token")
      window.sessionStorage.setItem("conductor.user", JSON.stringify(userValue))
    },
    { userValue: currentUser },
  )

  await page.route(/^https?:\/\/[^/]+\/api(?:\/|\?|$)/, async (route) => {
    const url = new URL(route.request().url())
    const path = url.pathname

    switch (path) {
      case "/api/setup/status":
        return json(route, {
          configured: true,
          project_name: "playwright-mission-control",
          display_name: "Playwright Mission Control",
          logo_url: null,
          public_url: "http://127.0.0.1:5181",
          sso_enabled: true,
        })
      case "/api/auth/me":
        return json(route, currentUser)
      case "/api/authorization/me":
        return json(route, authorizationProjection(role))
      case "/api/project":
        return json(route, {
          project_name: "playwright-mission-control",
          display_name: "Playwright Mission Control",
          description: "Deterministic Dashboard browser fixture",
          logo_url: null,
        })
      case "/api/members/pending/count":
        log.pendingCount += 1
        return json(route, { count: 2 })
      case "/api/dashboard":
        log.dashboard.push(route.request().url())
        await options.summaryGate
        if (options.summaryFailure) {
          return json(
            route,
            { error: options.summaryFailure.message },
            options.summaryFailure.status,
          )
        }
        return json(
          route,
          options.summary ??
            (role === "contribute"
              ? CONTRIBUTOR_DASHBOARD_SUMMARY
              : POPULATED_DASHBOARD_SUMMARY),
        )
      case "/api/analytics/resource-usage":
        log.analytics.push(route.request().url())
        await options.analyticsGate
        if (options.analyticsFailure) {
          return json(
            route,
            { error: options.analyticsFailure.message },
            options.analyticsFailure.status,
          )
        }
        // Contributor responses are redacted by the real server. Keeping member
        // rows here deliberately proves the UI permission gate is defense in depth.
        return json(
          route,
          url.searchParams.get("scope") === "governed"
            ? (options.governedAnalytics ??
              options.analytics ??
              GOVERNED_DASHBOARD_ANALYTICS)
            : (options.analytics ?? POPULATED_DASHBOARD_ANALYTICS),
        )
      case "/api/resources":
        return json(route, [])
      default:
        log.unexpected.push(`${route.request().method()} ${path}`)
        return json(route, { error: `Unexpected Dashboard fixture request: ${path}` }, 404)
    }
  })

  return log
}

function user(id: string, role: DashboardRole) {
  return {
    id,
    email: `playwright.${role}@example.test`,
    display_name: `Playwright ${role}`,
    primary_role: role,
    sub_role_ids: [],
    tag_ids: [],
    status: "active",
    must_change_password: false,
    last_seen_at: "2026-08-18T01:30:00Z",
    created_at: "2026-08-01T00:00:00Z",
  }
}

function authorizationProjection(role: DashboardRole) {
  const grants = ROLE_PERMISSIONS[role].map((permission) => ({
    permission,
    constraints: { kind: "any" },
    ...(permission === "telemetry.project.read"
      ? { response_projection: "aggregate_only" }
      : {}),
  }))
  return {
    schema_version: 1,
    policy_revision: "playwright-dashboard-v1",
    current_role: role,
    current_grants: grants,
    fixed_roles: [
      { role: "admin", grants: [] },
      { role: "contribute", grants: [] },
      { role: "user", grants: [] },
    ],
    permission_metadata: PERMISSIONS.map((key) => ({ key })),
    condition_metadata: CONDITION_METADATA,
  }
}

function usageDay(
  date: string,
  requests: number,
  successes: number,
  errors: number,
  blocked: number,
  cancelled: number,
  tokens: number,
  estimatedCost: number,
) {
  return {
    date,
    requests,
    successes,
    errors,
    blocked,
    cancelled,
    tokens_in: Math.round(tokens * 0.8),
    tokens_out: Math.round(tokens * 0.2),
    cache_read_tokens: Math.round(tokens * 0.16),
    reasoning_tokens: Math.round(tokens * 0.04),
    tool_use_tokens: Math.round(tokens * 0.02),
    estimated_cost_usd_micros: estimatedCost,
    unpriced_model_calls: 0,
  }
}

function tool(
  toolName: string,
  category: string,
  calls: number,
  successes: number,
  errors: number,
  blocked: number,
  cancelled: number,
  averageDurationMs: number,
) {
  return {
    tool_name: toolName,
    category,
    calls,
    successes,
    errors,
    blocked,
    cancelled,
    average_duration_ms: averageDurationMs,
    last_used_at: "2026-08-18T01:25:00Z",
  }
}

async function json(route: Route, body: unknown, status = 200) {
  await route.fulfill({
    status,
    contentType: "application/json",
    body: JSON.stringify(body),
  })
}
