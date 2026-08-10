import type {
  ClientPlatform,
  PrimaryRole,
  UserStatus,
} from "@/shared/constants/member"
import type { SecretScope } from "@/shared/constants/secret"
import type {
  TelemetryEventStatus,
  TelemetryEventType,
  TelemetryToolCategory,
} from "@/shared/constants/telemetry"
import { authSession } from "@/shared/lib/auth-session"

export type { PrimaryRole, UserStatus } from "@/shared/constants/member"
export type { SecretScope } from "@/shared/constants/secret"
export type SsoProvider = "oidc" | "github" | "azure_ad" | "google" | "custom"

export interface SetupStatus {
  configured: boolean
  project_name: string | null
  display_name: string | null
  logo_url: string | null
  public_url: string | null
  sso_enabled: boolean
}

export interface SetupRequest {
  project_name: string
  display_name?: string
  bind_host: string
  bind_port: number
  public_url?: string
  admin_email: string
  admin_display_name: string
  admin_password: string
  sso?: {
    enabled: boolean
    provider: SsoProvider
    issuer_url?: string
    client_id?: string
    client_secret?: string
    redirect_uri?: string
    scopes?: string[]
  }
}

export interface User {
  id: string
  email: string
  display_name: string
  primary_role: PrimaryRole
  sub_role_ids: string[]
  tag_ids: string[]
  status: UserStatus
  must_change_password: boolean
  last_seen_at: string | null
  created_at: string
}

export interface ClientInstallationSummary {
  id: string
  display_name: string
  platform: ClientPlatform
  evoflux_version: string
  connected_at: string
  last_seen_at: string
}

export interface ModelUsageBreakdown {
  provider: string
  model: string
  calls: number
  tokens_in: number
  tokens_out: number
  total_tokens: number
}

export interface DailyTokenUsage {
  date: string
  requests: number
  tokens_in: number
  tokens_out: number
  total_tokens: number
}

export interface MemberUsageSummary {
  from: string
  to: string
  total_requests: number
  model_calls: number
  tool_calls: number
  error_count: number
  tokens_in: number
  tokens_out: number
  total_tokens: number
  cache_read_tokens: number
  reasoning_tokens: number
  models: ModelUsageBreakdown[]
  daily: DailyTokenUsage[]
}

export interface MemberActivityItem {
  request_id: string
  session_id: string | null
  started_at: string
  finished_at: string
  provider: string | null
  model: string | null
  model_calls: number
  tool_calls: number
  tokens_in: number
  tokens_out: number
  total_tokens: number
  duration_ms: number
  status: TelemetryEventStatus
}

export interface MemberActivityResponse {
  items: MemberActivityItem[]
  total: number
  limit: number
  offset: number
}

export interface TelemetryEventDetail {
  event_id: string
  event_type: TelemetryEventType
  sequence: number
  agent_name: string | null
  provider: string | null
  model: string | null
  tokens_in: number
  tokens_out: number
  cache_read_tokens: number
  reasoning_tokens: number
  tool_use_tokens: number
  duration_ms: number
  tool_name: string | null
  tool_category: TelemetryToolCategory | null
  status: TelemetryEventStatus
  error_category: string | null
  reported_at: string
}

export interface MemberRequestDetail {
  request: MemberActivityItem
  events: TelemetryEventDetail[]
}

export interface MemberToolUsage {
  tool_name: string
  category: TelemetryToolCategory
  calls: number
  successes: number
  errors: number
  average_duration_ms: number
  last_used_at: string
}

export interface MemberToolsSummary {
  from: string
  to: string
  total_calls: number
  successful_calls: number
  failed_calls: number
  tools: MemberToolUsage[]
}

export interface DateRangeParams {
  from?: string
  to?: string
}

export interface AuthSession {
  token: string
  user: User
  expires_at: string
}

export interface MemberListResponse {
  items: User[]
  total: number
  page: number
  limit: number
}

export interface MemberListParams {
  q?: string
  status?: UserStatus | ""
  role?: PrimaryRole | ""
  tag?: string
  page?: number
  limit?: number
}

export interface CreateMemberBody {
  email: string
  display_name: string
  primary_role: PrimaryRole
  sub_role_ids?: string[]
  tag_ids?: string[]
}

export interface CreatedMember {
  user: User
  temporary_password: string
}

export interface UpdateMemberBody {
  display_name?: string
  primary_role?: PrimaryRole
  sub_role_ids?: string[]
  tag_ids?: string[]
}

export interface ApproveMemberBody {
  primary_role?: PrimaryRole
  sub_role_ids?: string[]
  tag_ids?: string[]
}

export interface DashboardSummary {
  project_name: string
  members_total: number
  members_online: number
  secrets_active: number
  resources: {
    agents: number
    skills: number
    mcp: number
    workflows: number
  }
  sso_enabled: boolean
}

export interface SubRole {
  id: string
  slug: string
  name: string
  description: string | null
  color: string | null
}

export interface Tag {
  id: string
  slug: string
  name: string
  description: string | null
  color: string | null
}

export interface SsoConfig {
  enabled: boolean
  provider: SsoProvider
  issuer_url: string | null
  client_id: string | null
  client_secret_set?: boolean
  redirect_uri: string | null
  scopes: string[]
}

export interface ProjectBranding {
  project_name: string
  display_name: string | null
  logo_url: string | null
}

export interface RealtimeSettings {
  max_connections: number
  max_connections_per_secret: number
  heartbeat_seconds: number
}

export interface ProjectSettings {
  project_name: string
  display_name: string | null
  bind_host: string
  bind_port: number
  public_url: string | null
  logo_url: string | null
  realtime: RealtimeSettings
  sso: SsoConfig
}

export interface ConnectionSecret {
  id: string
  name: string
  prefix: string
  owner_user_id: string
  scopes: SecretScope[]
  last_used_at: string | null
  expires_at: string | null
  revoked_at: string | null
  created_at: string
}

export interface CreatedSecret {
  secret: ConnectionSecret
  token: string
}

export interface ManagedResource {
  id: string
  kind: "agent" | "skill" | "mcp" | "workflow" | "command"
  slug: string
  name: string
  description: string | null
  version: string
  owner_user_id: string | null
  visibility: "shared" | "private"
  status: "draft" | "published" | "archived"
  payload: unknown
  published_at: string | null
  created_at: string
  updated_at: string
}

export interface ResourceVersion {
  id: string
  resource_id: string
  version: string
  status: "draft" | "published" | "deprecated"
  payload: unknown
  changelog: string | null
  created_by: string
  created_at: string
  published_at: string | null
}

export interface ResourceAccessPolicy {
  all_members: boolean
  primary_roles: string[]
  sub_role_ids: string[]
  tag_ids: string[]
  member_ids: string[]
}

export interface ResourceMonitoring {
  resource_id: string
  days: number
  summary: {
    executions: number
    successes: number
    failures: number
    active_members: number
    success_rate: number
    average_duration_ms: number
    tokens_in: number
    tokens_out: number
    feedback_count: number
    average_rating: number | null
  }
  daily: Array<{
    date: string
    executions: number
    successes: number
    failures: number
    average_duration_ms: number
  }>
  members: Array<{
    user_id: string
    member_name: string
    executions: number
    success_rate: number
    average_duration_ms: number
    last_used_at: string
  }>
}

export interface ResourceFeedback {
  id: string
  resource_id: string
  resource_version: string
  user_id: string
  member_name: string
  rating: number
  comment: string | null
  created_at: string
  updated_at: string
}

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const token = authSession.getToken()
  const headers = new Headers(init?.headers)
  headers.set("Content-Type", "application/json")
  if (token) headers.set("Authorization", `Bearer ${token}`)

  const res = await fetch(`/api${path}`, { ...init, headers })
  if (!res.ok) {
    let message = res.statusText
    try {
      const body = (await res.json()) as { error?: string }
      if (body.error) message = body.error
    } catch {
      /* ignore */
    }
    if (res.status === 401 && path !== "/auth/login") {
      authSession.clear()
      if (window.location.pathname !== "/login") {
        window.location.assign("/login?reason=session_expired")
      }
    }
    throw new ApiError(message, res.status)
  }
  if (res.status === 204) return undefined as T
  return res.json() as Promise<T>
}

export class ApiError extends Error {
  readonly status: number

  constructor(message: string, status: number) {
    super(message)
    this.name = "ApiError"
    this.status = status
  }
}

function qs(params: Record<string, string | number | undefined | null>): string {
  const search = new URLSearchParams()
  for (const [k, v] of Object.entries(params)) {
    if (v === undefined || v === null || v === "") continue
    search.set(k, String(v))
  }
  const s = search.toString()
  return s ? `?${s}` : ""
}

export const api = {
  health: () => request<{ status: string }>("/health"),
  setupStatus: () => request<SetupStatus>("/setup/status"),
  completeSetup: (body: SetupRequest) =>
    request<SetupStatus>("/setup", { method: "POST", body: JSON.stringify(body) }),
  login: (email: string, password: string) =>
    request<AuthSession>("/auth/login", {
      method: "POST",
      body: JSON.stringify({ email, password }),
    }),
  me: () => request<User>("/auth/me"),
  changePassword: (body: { current_password?: string; new_password: string }) =>
    request<AuthSession>("/auth/change-password", {
      method: "POST",
      body: JSON.stringify(body),
    }),
  dashboard: () => request<DashboardSummary>("/dashboard"),

  members: (params: MemberListParams = {}) =>
    request<MemberListResponse>(
      `/members${qs({
        q: params.q,
        status: params.status,
        role: params.role,
        tag: params.tag,
        page: params.page,
        limit: params.limit,
      })}`,
    ),
  pendingCount: () => request<{ count: number }>("/members/pending/count"),
  getMember: (id: string) => request<User>(`/members/${id}`),
  memberInstallations: (id: string) =>
    request<ClientInstallationSummary[]>(`/members/${id}/installations`),
  memberSecrets: (id: string) =>
    request<ConnectionSecret[]>(`/members/${id}/secrets`),
  createMemberSecret: (
    id: string,
    body: { name: string; scopes: SecretScope[]; expires_at?: string },
  ) =>
    request<CreatedSecret>(`/members/${id}/secrets`, {
      method: "POST",
      body: JSON.stringify(body),
    }),
  revokeMemberSecret: (memberId: string, secretId: string) =>
    request<{ revoked: boolean }>(`/members/${memberId}/secrets/${secretId}/revoke`, {
      method: "POST",
    }),
  memberUsageSummary: (id: string, params: DateRangeParams = {}) =>
    request<MemberUsageSummary>(
      `/members/${id}/usage/summary${qs({ from: params.from, to: params.to })}`,
    ),
  memberActivity: (
    id: string,
    params: DateRangeParams & { limit?: number; offset?: number } = {},
  ) =>
    request<MemberActivityResponse>(
      `/members/${id}/activity${qs({
        from: params.from,
        to: params.to,
        limit: params.limit,
        offset: params.offset,
      })}`,
    ),
  memberRequestDetail: (id: string, requestId: string) =>
    request<MemberRequestDetail>(
      `/members/${id}/activity/${encodeURIComponent(requestId)}`,
    ),
  memberTools: (id: string, params: DateRangeParams = {}) =>
    request<MemberToolsSummary>(
      `/members/${id}/tools${qs({ from: params.from, to: params.to })}`,
    ),
  createMember: (body: CreateMemberBody) =>
    request<CreatedMember>("/members", { method: "POST", body: JSON.stringify(body) }),
  updateMember: (id: string, body: UpdateMemberBody) =>
    request<User>(`/members/${id}`, { method: "PATCH", body: JSON.stringify(body) }),
  approveMember: (id: string, body: ApproveMemberBody = {}) =>
    request<User>(`/members/${id}/approve`, {
      method: "POST",
      body: JSON.stringify(body),
    }),
  disableMember: (id: string) =>
    request<User>(`/members/${id}/disable`, { method: "POST" }),
  enableMember: (id: string) =>
    request<User>(`/members/${id}/enable`, { method: "POST" }),
  resetMemberPassword: (id: string) =>
    request<{ temporary_password: string }>(`/members/${id}/reset-password`, {
      method: "POST",
    }),

  subRoles: () => request<SubRole[]>("/sub-roles"),
  createSubRole: (body: {
    slug: string
    name: string
    description?: string
    color?: string
  }) => request<SubRole>("/sub-roles", { method: "POST", body: JSON.stringify(body) }),
  updateSubRole: (
    id: string,
    body: { name?: string; description?: string; color?: string },
  ) =>
    request<SubRole>(`/sub-roles/${id}`, {
      method: "PATCH",
      body: JSON.stringify(body),
    }),
  deleteSubRole: (id: string) =>
    request<{ deleted: boolean }>(`/sub-roles/${id}`, { method: "DELETE" }),

  tags: () => request<Tag[]>("/tags"),
  createTag: (body: {
    slug: string
    name: string
    description?: string
    color?: string
  }) => request<Tag>("/tags", { method: "POST", body: JSON.stringify(body) }),
  updateTag: (
    id: string,
    body: { name?: string; description?: string; color?: string },
  ) => request<Tag>(`/tags/${id}`, { method: "PATCH", body: JSON.stringify(body) }),
  deleteTag: (id: string) =>
    request<{ deleted: boolean }>(`/tags/${id}`, { method: "DELETE" }),
  entityTags: (entityType: string, entityId: string) =>
    request<{ entity_type: string; entity_id: string; tag_ids: string[] }>(
      `/tag-assignments/${encodeURIComponent(entityType)}/${encodeURIComponent(entityId)}`,
    ),
  setEntityTags: (entityType: string, entityId: string, tagIds: string[]) =>
    request<{ entity_type: string; entity_id: string; tag_ids: string[] }>(
      `/tag-assignments/${encodeURIComponent(entityType)}/${encodeURIComponent(entityId)}`,
      { method: "PUT", body: JSON.stringify({ tag_ids: tagIds }) },
    ),

  project: () => request<ProjectBranding>("/project"),
  settings: () => request<ProjectSettings>("/settings"),
  updateSettings: (body: {
    project_name?: string
    display_name?: string
    public_url?: string
    logo_url?: string
  }) =>
    request<ProjectSettings>("/settings", {
      method: "PATCH",
      body: JSON.stringify(body),
    }),
  updateNetwork: (body: {
    bind_host: string
    bind_port: number
    public_url?: string
    realtime: RealtimeSettings
  }) =>
    request<ProjectSettings>("/settings/network", {
      method: "PUT",
      body: JSON.stringify(body),
    }),
  getSso: () => request<SsoConfig>("/sso"),
  updateSso: (body: {
    enabled: boolean
    provider: SsoProvider
    issuer_url?: string
    client_id?: string
    client_secret?: string
    redirect_uri?: string
    scopes?: string[]
  }) => request<SsoConfig>("/sso", { method: "PUT", body: JSON.stringify(body) }),

  secrets: () => request<ConnectionSecret[]>("/secrets"),
  createSecret: (body: {
    name: string
    scopes: SecretScope[]
    expires_at?: string
  }) =>
    request<CreatedSecret>("/secrets", { method: "POST", body: JSON.stringify(body) }),
  revokeSecret: (id: string) =>
    request<{ revoked: boolean }>(`/secrets/${id}/revoke`, { method: "POST" }),
  resources: () => request<ManagedResource[]>("/resources"),
  createResource: (body: {
    kind: ManagedResource["kind"]
    slug: string
    name: string
    description?: string
    version: string
    visibility: ManagedResource["visibility"]
    payload: unknown
    changelog?: string
  }) =>
    request<ManagedResource>("/resources", {
      method: "POST",
      body: JSON.stringify(body),
    }),
  updateResource: (
    id: string,
    body: {
      name?: string
      description?: string
      visibility?: ManagedResource["visibility"]
    },
  ) =>
    request<ManagedResource>(`/resources/${id}`, {
      method: "PATCH",
      body: JSON.stringify(body),
    }),
  archiveResource: (id: string) =>
    request<ManagedResource>(`/resources/${id}/archive`, { method: "POST" }),
  resourceVersions: (id: string) =>
    request<ResourceVersion[]>(`/resources/${id}/versions`),
  createResourceVersion: (
    id: string,
    body: { version: string; payload: unknown; changelog?: string },
  ) =>
    request<ResourceVersion>(`/resources/${id}/versions`, {
      method: "POST",
      body: JSON.stringify(body),
    }),
  publishResourceVersion: (resourceId: string, versionId: string) =>
    request<ManagedResource>(
      `/resources/${resourceId}/versions/${versionId}/publish`,
      { method: "POST" },
    ),
  resourceAccess: (id: string) =>
    request<ResourceAccessPolicy>(`/resources/${id}/access`),
  setResourceAccess: (id: string, body: ResourceAccessPolicy) =>
    request<ResourceAccessPolicy>(`/resources/${id}/access`, {
      method: "PUT",
      body: JSON.stringify(body),
    }),
  resourceMonitoring: (id: string, days = 30) =>
    request<ResourceMonitoring>(`/resources/${id}/monitoring?days=${days}`),
  resourceFeedback: (id: string) =>
    request<ResourceFeedback[]>(`/resources/${id}/feedback`),
  submitResourceFeedback: (id: string, rating: number, comment?: string) =>
    request<ResourceFeedback>(`/resources/${id}/feedback`, {
      method: "PUT",
      body: JSON.stringify({ rating, comment }),
    }),
  ssoStart: () =>
    request<{ authorization_url: string; provider: string }>("/auth/sso/start"),
}
