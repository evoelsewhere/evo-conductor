export type PrimaryRole = "admin" | "contribute" | "user"
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
  status: "active" | "invited" | "disabled"
  last_seen_at: string | null
  created_at: string
}

export interface AuthSession {
  token: string
  user: User
  expires_at: string
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
  return res.json() as Promise<T>
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
  dashboard: () => request<DashboardSummary>("/dashboard"),
  members: () => request<User[]>("/members"),
  subRoles: () => request<SubRole[]>("/sub-roles"),
  createSubRole: (body: { slug: string; name: string; description?: string; color?: string }) =>
    request<SubRole>("/sub-roles", { method: "POST", body: JSON.stringify(body) }),
  secrets: () => request<ConnectionSecret[]>("/secrets"),
  createSecret: (body: { name: string; scopes: SecretScope[] }) =>
    request<CreatedSecret>("/secrets", { method: "POST", body: JSON.stringify(body) }),
  revokeSecret: (id: string) =>
    request<{ revoked: boolean }>(`/secrets/${id}/revoke`, { method: "POST" }),
  resources: () => request<ManagedResource[]>("/resources"),
  ssoStart: () =>
    request<{ authorization_url: string; provider: string }>("/auth/sso/start"),
}
