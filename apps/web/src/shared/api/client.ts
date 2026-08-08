export type PrimaryRole = "admin" | "contribute" | "user"
export type UserStatus = "pending" | "invited" | "active" | "disabled"
export type SsoProvider = "oidc" | "github" | "azure_ad" | "google" | "custom"
export type SecretScope = "subscribe_resources" | "report_telemetry" | "sync_inventory"

export interface SetupStatus {
  configured: boolean
  project_name: string | null
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

export interface ProjectSettings {
  project_name: string
  display_name: string | null
  bind_host: string
  bind_port: number
  public_url: string | null
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
  visibility: "shared" | "private"
}

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const token = localStorage.getItem("conductor.token")
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
    throw new Error(message)
  }
  if (res.status === 204) return undefined as T
  return res.json() as Promise<T>
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
    request<User>("/auth/change-password", {
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

  settings: () => request<ProjectSettings>("/settings"),
  updateSettings: (body: {
    project_name?: string
    display_name?: string
    public_url?: string
  }) =>
    request<ProjectSettings>("/settings", {
      method: "PATCH",
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
  createSecret: (body: { name: string; scopes: SecretScope[] }) =>
    request<CreatedSecret>("/secrets", { method: "POST", body: JSON.stringify(body) }),
  revokeSecret: (id: string) =>
    request<{ revoked: boolean }>(`/secrets/${id}/revoke`, { method: "POST" }),
  resources: () => request<ManagedResource[]>("/resources"),
  ssoStart: () =>
    request<{ authorization_url: string; provider: string }>("/auth/sso/start"),
}
