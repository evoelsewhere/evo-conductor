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
import type {
  ReleaseChannel,
  ResourceKind,
  ResourceStatus,
  ResourceTargetMode,
  ResourceVersionStatus,
  VersionMode,
} from "@/shared/constants/resource"
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
  estimated_cost_usd_micros: number
  unpriced_model_calls: number
  status: TelemetryEventStatus
}

export interface TelemetryResourceAttributionDetail {
  resource_id: string
  version_id: string
  kind: ResourceKind
  name: string
  version: string
  relation: TelemetryResourceRelation
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
  response_model: string | null
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
  estimated_cost_usd_micros: number | null
  cost_source: "evoflux_catalog" | null
  resources: TelemetryResourceAttributionDetail[]
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

export type TelemetryResourceRelation =
  | "executing_agent"
  | "activated_skill"
  | "plugin_contributed_skill"
  | "plugin_contributed_tool"

export interface ResourceUsageTotals {
  reported_installations: number
  installed_installations: number
  installed_members: number
  pending_installations: number
  attention_installations: number
  requests: number
  resource_uses: number
  model_calls: number
  tool_calls: number
  successes: number
  errors: number
  blocked: number
  cancelled: number
  tokens_in: number
  tokens_out: number
  cache_read_tokens: number
  reasoning_tokens: number
  tool_use_tokens: number
  total_tokens: number
  estimated_cost_usd_micros: number
  unpriced_model_calls: number
  average_tokens_per_request: number
  average_duration_ms: number
}

export interface ResourceUsageDay {
  date: string
  requests: number
  successes: number
  errors: number
  blocked: number
  cancelled: number
  tokens_in: number
  tokens_out: number
  cache_read_tokens: number
  reasoning_tokens: number
  tool_use_tokens: number
  estimated_cost_usd_micros: number
  unpriced_model_calls: number
}

export interface ResourceUsageBreakdown {
  resource_id: string
  version_id: string
  kind: ResourceKind
  name: string
  version: string
  relation: TelemetryResourceRelation
  uses: number
  members: number
  requests: number
  successes: number
  errors: number
  model_calls: number
  tool_calls: number
  total_tokens: number
  estimated_cost_usd_micros: number
  last_used_at: string
}

export interface ResourceUsageMember {
  user_id: string
  display_name: string
  email: string
  primary_role: PrimaryRole
  requests: number
  resource_uses: number
  model_calls: number
  tool_calls: number
  installations: number
  total_tokens: number
  estimated_cost_usd_micros: number
  last_received_at: string
}

export interface ResourceUsageModel {
  provider: string
  model: string
  calls: number
  total_tokens: number
  estimated_cost_usd_micros: number
  unpriced_calls: number
}

export interface ResourceUsageRole {
  primary_role: PrimaryRole
  requests: number
  model_calls: number
  tool_calls: number
  total_tokens: number
  estimated_cost_usd_micros: number
}

export interface ResourceUsageTool {
  tool_name: string
  category: TelemetryToolCategory
  calls: number
  successes: number
  errors: number
  blocked: number
  cancelled: number
  average_duration_ms: number
  last_used_at: string
}

export interface ResourceUsageActivityItem {
  request_id: string
  user_id: string
  display_name: string
  primary_role: PrimaryRole
  resource_id: string
  version_id: string
  kind: ResourceKind
  resource_name: string
  version: string
  relation: TelemetryResourceRelation
  occurred_at: string
  status: TelemetryEventStatus
  provider: string | null
  model: string | null
  model_calls: number
  tool_calls: number
  total_tokens: number
  estimated_cost_usd_micros: number
  unpriced_model_calls: number
  duration_ms: number
}

export interface ResourceUsageAnalytics {
  from: string
  to: string
  totals: ResourceUsageTotals
  daily: ResourceUsageDay[]
  resources: ResourceUsageBreakdown[]
  members: ResourceUsageMember[]
  models: ResourceUsageModel[]
  roles: ResourceUsageRole[]
  tools: ResourceUsageTool[]
  activity: ResourceUsageActivityItem[]
  activity_total: number
  limit: number
  offset: number
}

export interface ResourceUsageParams extends DateRangeParams {
  member_id?: string
  primary_role?: PrimaryRole
  resource_kind?: ResourceKind
  resource_id?: string
  version_id?: string
  status?: TelemetryEventStatus
  provider?: string
  model?: string
  installation_id?: string
  relation?: TelemetryResourceRelation
  tool_name?: string
  limit?: number
  offset?: number
}

export type AnalyticsViewVisibility = "private" | "shared"
export type AnalyticsDateRange =
  | "last_24_hours"
  | "last_7_days"
  | "last_30_days"
  | "last_90_days"
  | "custom"
export type AnalyticsComparison =
  | "previous_period"
  | "previous_week"
  | "previous_month"
export type AnalyticsDashboardPreset =
  | "executive"
  | "adoption"
  | "reliability"
  | "cost"
  | "custom"
export type AnalyticsDashboardDensity = "comfortable" | "compact"
export type AnalyticsMetric =
  | "requests"
  | "resource_uses"
  | "model_calls"
  | "tool_calls"
  | "input_tokens"
  | "output_tokens"
  | "total_tokens"
  | "estimated_cost"
  | "success_rate"
  | "error_rate"
  | "average_duration"
  | "installations"
  | "feedback_rating"
export type AnalyticsDimension =
  | "time"
  | "outcome"
  | "resource"
  | "resource_kind"
  | "version"
  | "member"
  | "role"
  | "provider"
  | "model"
  | "tool"
  | "installation"
export type AnalyticsVisualization =
  | "kpi"
  | "line"
  | "area"
  | "bar"
  | "stacked_bar"
  | "donut"
  | "table"
export type AnalyticsWidgetSize = "one_third" | "half" | "two_thirds" | "full"

export interface AnalyticsQuery {
  date_range: AnalyticsDateRange
  from?: string | null
  to?: string | null
  comparison?: AnalyticsComparison | null
  member_id?: string | null
  primary_role?: PrimaryRole | null
  resource_kind?: ResourceKind | null
  resource_id?: string | null
  version_id?: string | null
  status?: TelemetryEventStatus | null
  provider?: string | null
  model?: string | null
  installation_id?: string | null
  relation?: TelemetryResourceRelation | null
  tool_name?: string | null
}

export interface AnalyticsWidget {
  id: string
  title: string
  visualization: AnalyticsVisualization
  metric: AnalyticsMetric
  group_by: AnalyticsDimension | null
  size: AnalyticsWidgetSize
  limit: number
  show_legend: boolean
}

export interface AnalyticsViewDefinition {
  schema_version: 1
  preset: AnalyticsDashboardPreset
  density: AnalyticsDashboardDensity
  query: AnalyticsQuery
  widgets: AnalyticsWidget[]
}

export interface AnalyticsView {
  id: string
  project_id: string
  owner_user_id: string
  name: string
  description: string | null
  visibility: AnalyticsViewVisibility
  definition: AnalyticsViewDefinition
  revision: number
  created_at: string
  updated_at: string
}

export interface CreateAnalyticsViewBody {
  name: string
  description?: string | null
  visibility: AnalyticsViewVisibility
  definition: AnalyticsViewDefinition
}

export interface UpdateAnalyticsViewBody extends CreateAnalyticsViewBody {
  revision: number
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

export const AUTHORIZATION_PERMISSION_KEYS = [
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

export type PermissionKey = (typeof AUTHORIZATION_PERMISSION_KEYS)[number]
export type AuthorizationLifecycle =
  | "draft"
  | "beta"
  | "published"
  | "archived"
  | "deprecated"
export type AuthorizationResponseProjection =
  | "full"
  | "directory_safe"
  | "aggregate_only"

export type AuthorizationConstraint =
  | { kind: "any" }
  | { kind: "self" }
  | { kind: "owner" }
  | { kind: "effective_audience" }
  | { kind: "same_project" }
  | { kind: "resource_kind_in"; values: ResourceKind[] }
  | { kind: "lifecycle_in"; values: AuthorizationLifecycle[] }
  | { kind: "all_of"; items: AuthorizationConstraint[] }
  | { kind: "any_of"; items: AuthorizationConstraint[] }

export interface PermissionGrant {
  permission: PermissionKey
  constraints: AuthorizationConstraint
  response_projection?: AuthorizationResponseProjection
}

export interface FixedRolePolicy {
  role: PrimaryRole
  grants: PermissionGrant[]
}

export interface PermissionMetadata {
  key: PermissionKey
}

export interface ConditionMetadata {
  kind:
    | "any"
    | "self"
    | "owner"
    | "resource_kind_in"
    | "lifecycle_in"
    | "same_project"
    | "effective_audience"
  evaluation: "ui_target_context" | "server_only"
}

export interface AuthorizationProjection {
  schema_version: 1
  policy_revision: string
  current_role: PrimaryRole
  current_grants: PermissionGrant[]
  fixed_roles: FixedRolePolicy[]
  permission_metadata: PermissionMetadata[]
  condition_metadata: ConditionMetadata[]
}

export interface DirectoryMember {
  id: string
  display_name: string
  primary_role: PrimaryRole
}

export type MemberListItem = User | DirectoryMember

export interface MemberListResponse {
  items: MemberListItem[]
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
    plugins: number
    workflows: number
  }
  sso_enabled: boolean
  presence: {
    clients_seen_recently: number
    members_seen_recently: number
    threshold_seconds: number
    observed_at: string
  }
  realtime: {
    active_owners: number
    active_streams: number
    scope: "this_node"
  }
  host_metrics: {
    cpu_usage_percent?: number | null
    memory_used_bytes?: number | null
    memory_total_bytes?: number | null
    gpu_usage_percent?: number | null
    vram_used_bytes?: number | null
    vram_total_bytes?: number | null
    sampled_at: string
    scope: "conductor_host"
  }
  feedback: {
    scope: "project" | "owned_resources"
    count: number
    average_rating?: number | null
    positive_count: number
    positive_percent?: number | null
    distribution: {
      rating_1: number
      rating_2: number
      rating_3: number
      rating_4: number
      rating_5: number
    }
  }
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
  description: string | null
  logo_url: string | null
}

export interface RealtimeSettings {
  max_connections: number
  max_connections_per_secret: number
  heartbeat_seconds: number
}

export type StorageBackend = "local" | "s3" | "azure_blob" | "git"
export type GitAuthMode = "environment" | "https_token"

export interface StorageSettings {
  backend: StorageBackend
  local: { root: string | null }
  s3: {
    bucket: string
    region: string
    endpoint: string | null
    prefix: string
    path_style: boolean
  }
  azure_blob: {
    account: string
    container: string
    endpoint: string | null
    prefix: string
  }
  git: {
    repository_url: string
    branch: string
    prefix: string
    auth_mode: GitAuthMode
    username: string | null
    credential?: string | null
    clear_credential?: boolean
    credential_set: boolean
  }
}

export interface StorageMigrationResult {
  storage: StorageSettings
  objects_copied: number
  bytes_copied: number
}

export type CollectionLevel = "L0" | "L1" | "L2"

export interface DataPolicySettings {
  collection_level: CollectionLevel
}

export interface ProjectSettings {
  project_name: string
  display_name: string | null
  description: string | null
  bind_host: string
  bind_port: number
  public_url: string | null
  logo_url: string | null
  realtime: RealtimeSettings
  data_policy: DataPolicySettings
  sso: SsoConfig
  storage: StorageSettings
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
  project_id: string
  kind: ResourceKind
  slug: string
  name: string
  description: string | null
  version: string
  highest_version: string | null
  draft_revision: number
  release_channel: ReleaseChannel | null
  owner_user_id: string | null
  visibility: "shared" | "private"
  status: ResourceStatus
  payload: unknown
  published_at: string | null
  created_at: string
  updated_at: string
}

export type ResourceBundleKind = Extract<ResourceKind, "agent" | "skill" | "plugin">

export interface FileManifestEntry {
  path: string
  sha256: string
  size: number
  media_type: string
  executable: boolean
}

export interface ResourceBundle {
  schema_version: 2
  kind: ResourceBundleKind
  slug: string
  version: string
  artifact_sha256: string
  artifact_size: number
  artifact_media_type: string
  tree_sha256: string
  files: FileManifestEntry[]
}

export interface ResourceVersion {
  id: string
  project_id: string
  resource_id: string
  version: string
  status: ResourceVersionStatus
  payload: unknown
  changelog: string | null
  release_channel: ReleaseChannel | null
  active_channel: ReleaseChannel | null
  content_sha256: string
  content_size: number
  artifact_key: string | null
  bundle?: ResourceBundle
  minimum_evoflux_version: string | null
  created_by: string
  created_at: string
  published_at: string | null
  deprecated_at: string | null
  deprecated_by: string | null
  deprecation_reason: string | null
}

export interface DraftFile {
  path: string
  content: string
}

export interface DraftFileTree {
  resource_id: string
  revision: number
  files: DraftFile[]
}

export interface ResourceDiagnostic {
  severity: "warning" | "error"
  code: string
  message: string
  path: string | null
  line: number | null
}

export interface ResourceValidation {
  valid: boolean
  revision: number
  diagnostics: ResourceDiagnostic[]
}

export interface DraftImportResponse {
  tree: DraftFileTree
  validation: ResourceValidation
}

export interface PluginArchiveManifest {
  name: string | null
  version: string | null
  description: string | null
}

export interface PluginArchiveInspection {
  manifest: PluginArchiveManifest
  validation: ResourceValidation
  file_count: number
  total_uncompressed_bytes: number
  skill_count: number
}

export interface PluginArchiveCreateResponse {
  resource: ManagedResource
  validation: ResourceValidation
}

export interface ResourceArchiveMetadata {
  slug: string | null
  version: string | null
  description: string | null
  primary_source: string | null
}

export interface ResourceArchiveInspection {
  kind: ResourceKind
  metadata: ResourceArchiveMetadata
  validation: ResourceValidation
  file_count: number
  total_uncompressed_bytes: number
}

export interface ResourceArchiveCreateResponse {
  resource: ManagedResource
  validation: ResourceValidation
}

export interface ReleaseResourceRequest {
  channel: ReleaseChannel
  version_mode: VersionMode
  manual_version: string | null
  draft_revision: number
  changelog: string | null
  beta_member_ids: string[]
  minimum_evoflux_version: string | null
}

export interface ReleaseResourceResult {
  resource_id: string
  version_id: string
  version: string
  channel: ReleaseChannel
  sha256: string
  size: number
  highest_version: string
  next_version: string
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

export interface ResourceInventoryMonitoring {
  resource_id: string
  summary: {
    reported_installations: number
    installed_installations: number
    installed_members: number
    pending_installations: number
    attention_installations: number
  }
  installations: Array<{
    installation_id: string
    installation_name: string
    platform: string
    evoflux_version: string
    user_id: string
    member_name: string
    email: string
    primary_role: PrimaryRole
    desired_version_id: string | null
    desired_version: string | null
    applied_version_id: string | null
    applied_version: string | null
    release_channel: ReleaseChannel | null
    plugin_installation_id: string | null
    observed_state: string
    error_category: string | null
    observed_at: string
    last_seen_at: string
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

const PRIMARY_ROLES = new Set<string>(["admin", "contribute", "user"])
const RESOURCE_KINDS = new Set<string>([
  "agent",
  "skill",
  "plugin",
  "workflow",
  "command",
])
const AUTHORIZATION_LIFECYCLES = new Set<string>([
  "draft",
  "beta",
  "published",
  "archived",
  "deprecated",
])
const RESPONSE_PROJECTIONS = new Set<string>([
  "full",
  "directory_safe",
  "aggregate_only",
])
const PERMISSION_KEYS = new Set<string>(AUTHORIZATION_PERMISSION_KEYS)
const CONDITION_EVALUATION = {
  any: "ui_target_context",
  self: "ui_target_context",
  owner: "ui_target_context",
  resource_kind_in: "ui_target_context",
  lifecycle_in: "ui_target_context",
  same_project: "server_only",
  effective_audience: "server_only",
} as const

function authorizationSchemaError(detail: string): Error {
  return new Error(`Authorization policy could not be loaded: ${detail}.`)
}

function objectValue(value: unknown, detail: string): Record<string, unknown> {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw authorizationSchemaError(detail)
  }
  return value as Record<string, unknown>
}

function stringValue(value: unknown, detail: string): string {
  if (typeof value !== "string" || !value.trim()) {
    throw authorizationSchemaError(detail)
  }
  return value
}

function enumValue<T extends string>(
  value: unknown,
  allowed: ReadonlySet<string>,
  detail: string,
): T {
  const parsed = stringValue(value, detail)
  if (!allowed.has(parsed)) throw authorizationSchemaError(detail)
  return parsed as T
}

function uniqueEnumArray<T extends string>(
  value: unknown,
  allowed: ReadonlySet<string>,
  detail: string,
): T[] {
  if (!Array.isArray(value) || value.length === 0) {
    throw authorizationSchemaError(detail)
  }
  const parsed = value.map((item) => enumValue<T>(item, allowed, detail))
  if (new Set(parsed).size !== parsed.length) throw authorizationSchemaError(detail)
  return parsed
}

function parseConstraint(value: unknown): AuthorizationConstraint {
  const record = objectValue(value, "invalid permission constraint")
  const kind = stringValue(record.kind, "invalid permission constraint kind")
  switch (kind) {
    case "any":
    case "self":
    case "owner":
    case "effective_audience":
    case "same_project":
      return { kind }
    case "resource_kind_in":
      return {
        kind,
        values: uniqueEnumArray<ResourceKind>(
          record.values,
          RESOURCE_KINDS,
          "invalid resource kind constraint",
        ),
      }
    case "lifecycle_in":
      return {
        kind,
        values: uniqueEnumArray<AuthorizationLifecycle>(
          record.values,
          AUTHORIZATION_LIFECYCLES,
          "invalid lifecycle constraint",
        ),
      }
    case "all_of":
    case "any_of": {
      if (!Array.isArray(record.items) || record.items.length === 0) {
        throw authorizationSchemaError(`empty ${kind} constraint`)
      }
      return { kind, items: record.items.map(parseConstraint) }
    }
    default:
      throw authorizationSchemaError("unknown permission constraint")
  }
}

function parseGrant(value: unknown): PermissionGrant {
  const record = objectValue(value, "invalid permission grant")
  const permission = enumValue<PermissionKey>(
    record.permission,
    PERMISSION_KEYS,
    "unknown permission",
  )
  const responseProjection = record.response_projection === undefined
    ? undefined
    : enumValue<AuthorizationResponseProjection>(
        record.response_projection,
        RESPONSE_PROJECTIONS,
        "invalid response projection",
      )
  return {
    permission,
    constraints: parseConstraint(record.constraints),
    ...(responseProjection ? { response_projection: responseProjection } : {}),
  }
}

function parseGrantArray(value: unknown, detail: string): PermissionGrant[] {
  if (!Array.isArray(value)) throw authorizationSchemaError(detail)
  const grants = value.map(parseGrant)
  if (new Set(grants.map((grant) => grant.permission)).size !== grants.length) {
    throw authorizationSchemaError("duplicate permission grant")
  }
  return grants
}

export function parseAuthorizationProjection(value: unknown): AuthorizationProjection {
  const record = objectValue(value, "invalid response")
  if (record.schema_version !== 1) {
    throw authorizationSchemaError("unsupported schema version")
  }
  const currentRole = enumValue<PrimaryRole>(
    record.current_role,
    PRIMARY_ROLES,
    "invalid current role",
  )
  const currentGrants = parseGrantArray(record.current_grants, "invalid current grants")

  if (!Array.isArray(record.fixed_roles) || record.fixed_roles.length !== 3) {
    throw authorizationSchemaError("incomplete fixed role metadata")
  }
  const fixedRoles = record.fixed_roles.map((item): FixedRolePolicy => {
    const role = objectValue(item, "invalid fixed role")
    return {
      role: enumValue<PrimaryRole>(role.role, PRIMARY_ROLES, "invalid fixed role"),
      grants: parseGrantArray(role.grants, "invalid fixed role grants"),
    }
  })
  if (new Set(fixedRoles.map((item) => item.role)).size !== PRIMARY_ROLES.size) {
    throw authorizationSchemaError("duplicate fixed role metadata")
  }

  if (!Array.isArray(record.permission_metadata)) {
    throw authorizationSchemaError("invalid permission metadata")
  }
  const permissionMetadata = record.permission_metadata.map((item): PermissionMetadata => {
    const metadata = objectValue(item, "invalid permission metadata")
    return {
      key: enumValue<PermissionKey>(metadata.key, PERMISSION_KEYS, "unknown permission metadata"),
    }
  })
  if (
    permissionMetadata.length !== AUTHORIZATION_PERMISSION_KEYS.length ||
    new Set(permissionMetadata.map((item) => item.key)).size !== PERMISSION_KEYS.size
  ) {
    throw authorizationSchemaError("incomplete permission metadata")
  }

  if (!Array.isArray(record.condition_metadata)) {
    throw authorizationSchemaError("invalid condition metadata")
  }
  const conditionMetadata = record.condition_metadata.map((item): ConditionMetadata => {
    const metadata = objectValue(item, "invalid condition metadata")
    const kind = stringValue(metadata.kind, "invalid condition kind")
    if (!(kind in CONDITION_EVALUATION)) {
      throw authorizationSchemaError("unknown condition metadata")
    }
    const typedKind = kind as keyof typeof CONDITION_EVALUATION
    const evaluation = CONDITION_EVALUATION[typedKind]
    if (metadata.evaluation !== evaluation) {
      throw authorizationSchemaError("invalid condition evaluation")
    }
    return { kind: typedKind, evaluation }
  })
  if (
    conditionMetadata.length !== Object.keys(CONDITION_EVALUATION).length ||
    new Set(conditionMetadata.map((item) => item.kind)).size !== conditionMetadata.length
  ) {
    throw authorizationSchemaError("incomplete condition metadata")
  }

  return {
    schema_version: 1,
    policy_revision: stringValue(record.policy_revision, "invalid policy revision"),
    current_role: currentRole,
    current_grants: currentGrants,
    fixed_roles: fixedRoles,
    permission_metadata: permissionMetadata,
    condition_metadata: conditionMetadata,
  }
}

interface AuthorizationFailureHandler {
  onUnauthorized: () => void
  refreshAfterForbidden: () => Promise<void>
}

let authorizationFailureHandler: AuthorizationFailureHandler | null = null

export function configureAuthorizationFailureHandler(
  handler: AuthorizationFailureHandler,
) {
  authorizationFailureHandler = handler
}

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const token = authSession.getToken()
  const headers = new Headers(init?.headers)
  if (typeof init?.body === "string" && !headers.has("Content-Type")) {
    headers.set("Content-Type", "application/json")
  }
  if (token) headers.set("Authorization", `Bearer ${token}`)

  const res = await fetch(`/api${path}`, { ...init, headers })
  if (!res.ok) {
    let message = res.statusText
    let errorCode: string | null = null
    let requestId = res.headers.get("X-Request-ID")
    try {
      const body = objectValue(await res.json(), "invalid error response")
      if (typeof body.error === "string") message = body.error
      if (typeof body.error_code === "string") errorCode = body.error_code
      if (typeof body.request_id === "string") requestId = body.request_id
    } catch {
      /* ignore */
    }
    if (res.status === 401 && path !== "/auth/login") {
      authSession.clear()
      authorizationFailureHandler?.onUnauthorized()
      if (window.location.pathname !== "/login") {
        window.location.assign("/login?reason=session_expired")
      }
    }
    if (res.status === 403 && path !== "/authorization/me") {
      try {
        await authorizationFailureHandler?.refreshAfterForbidden()
      } catch {
        // Preserve the original denial; the store exposes any projection refresh failure.
      }
    }
    throw new ApiError(message, res.status, errorCode, requestId)
  }
  if (res.status === 204) return undefined as T
  return res.json() as Promise<T>
}

export class ApiError extends Error {
  readonly status: number
  readonly errorCode: string | null
  readonly requestId: string | null

  constructor(
    message: string,
    status: number,
    errorCode: string | null = null,
    requestId: string | null = null,
  ) {
    super(requestId ? `${message} \u00b7 Request ${requestId}` : message)
    this.name = "ApiError"
    this.status = status
    this.errorCode = errorCode
    this.requestId = requestId
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
  authorizationMe: async () =>
    parseAuthorizationProjection(await request<unknown>("/authorization/me")),
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
  resourceUsage: (params: ResourceUsageParams = {}) =>
    request<ResourceUsageAnalytics>(
      `/analytics/resource-usage${qs({
        from: params.from,
        to: params.to,
        member_id: params.member_id,
        primary_role: params.primary_role,
        resource_kind: params.resource_kind,
        resource_id: params.resource_id,
        version_id: params.version_id,
        status: params.status,
        provider: params.provider,
        model: params.model,
        installation_id: params.installation_id,
        relation: params.relation,
        tool_name: params.tool_name,
        limit: params.limit,
        offset: params.offset,
      })}`,
    ),
  analyticsViews: () => request<AnalyticsView[]>("/analytics/views"),
  analyticsView: (id: string) =>
    request<AnalyticsView>(`/analytics/views/${encodeURIComponent(id)}`),
  createAnalyticsView: (body: CreateAnalyticsViewBody) =>
    request<AnalyticsView>("/analytics/views", {
      method: "POST",
      body: JSON.stringify(body),
    }),
  updateAnalyticsView: (id: string, body: UpdateAnalyticsViewBody) =>
    request<AnalyticsView>(`/analytics/views/${encodeURIComponent(id)}`, {
      method: "PUT",
      body: JSON.stringify(body),
    }),
  deleteAnalyticsView: (id: string, revision: number) =>
    request<{ deleted: boolean }>(
      `/analytics/views/${encodeURIComponent(id)}${qs({ revision })}`,
      { method: "DELETE" },
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
    description?: string
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
  updateDataPolicy: (collectionLevel: CollectionLevel) =>
    request<ProjectSettings>("/settings/data-policy", {
      method: "PUT",
      body: JSON.stringify({ collection_level: collectionLevel }),
    }),
  updateStorage: (storage: StorageSettings, migrateExisting = true) =>
    request<StorageMigrationResult>("/settings/storage", {
      method: "PUT",
      body: JSON.stringify({
        storage,
        migrate_existing: migrateExisting,
      }),
    }),
  uploadProjectLogo: (file: File) =>
    request<ProjectSettings>("/settings/logo", {
      method: "PUT",
      headers: { "Content-Type": file.type || "application/octet-stream" },
      body: file,
    }),
  deleteProjectLogo: () =>
    request<ProjectSettings>("/settings/logo", { method: "DELETE" }),
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
    modes?: ResourceTargetMode[]
    changelog?: string
  }) =>
    request<ManagedResource>("/resources", {
      method: "POST",
      body: JSON.stringify(body),
    }),
  inspectPluginArchive: (file: File) =>
    request<PluginArchiveInspection>("/resources/plugins/inspect", {
      method: "POST",
      headers: { "Content-Type": file.type || "application/zip" },
      body: file,
    }),
  createPluginFromArchive: (
    file: File,
    body: { name: string; visibility: ManagedResource["visibility"] },
  ) =>
    request<PluginArchiveCreateResponse>(
      `/resources/plugins/import${qs({ name: body.name, visibility: body.visibility })}`,
      {
        method: "POST",
        headers: { "Content-Type": file.type || "application/zip" },
        body: file,
      },
    ),
  inspectResourceArchive: (kind: ResourceKind, file: File) =>
    request<ResourceArchiveInspection>(`/resources/imports/${kind}/inspect`, {
      method: "POST",
      headers: { "Content-Type": file.type || "application/zip" },
      body: file,
    }),
  createResourceFromArchive: (
    kind: ResourceKind,
    file: File,
    body: {
      slug: string
      name: string
      visibility: ManagedResource["visibility"]
      modes: ResourceTargetMode[]
    },
  ) =>
    request<ResourceArchiveCreateResponse>(
      `/resources/imports/${kind}${qs({
        slug: body.slug,
        name: body.name,
        visibility: body.visibility,
        modes: body.modes.join(","),
      })}`,
      {
        method: "POST",
        headers: { "Content-Type": file.type || "application/zip" },
        body: file,
      },
    ),
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
  resourceDraft: (id: string) =>
    request<DraftFileTree>(`/resources/${id}/draft/files`),
  saveResourceDraftFile: (
    id: string,
    path: string,
    content: string,
    draftRevision: number,
  ) =>
    request<DraftFileTree>(
      `/resources/${id}/draft/files/${path
        .split("/")
        .map((segment) => encodeURIComponent(segment))
        .join("/")}`,
      {
        method: "PUT",
        body: JSON.stringify({ content, draft_revision: draftRevision }),
      },
    ),
  createResourceDraftFile: (
    id: string,
    path: string,
    content: string,
    draftRevision: number,
  ) =>
    request<DraftFileTree>(`/resources/${id}/draft/entries`, {
      method: "POST",
      body: JSON.stringify({ path, content, draft_revision: draftRevision }),
    }),
  moveResourceDraftEntry: (
    id: string,
    path: string,
    destinationPath: string,
    draftRevision: number,
  ) =>
    request<DraftFileTree>(`/resources/${id}/draft/entries`, {
      method: "PATCH",
      body: JSON.stringify({
        path,
        destination_path: destinationPath,
        draft_revision: draftRevision,
      }),
    }),
  deleteResourceDraftEntry: (id: string, path: string, draftRevision: number) =>
    request<DraftFileTree>(`/resources/${id}/draft/entries`, {
      method: "DELETE",
      body: JSON.stringify({ path, draft_revision: draftRevision }),
    }),
  importResourceDraft: (id: string, file: File, draftRevision: number) =>
    request<DraftImportResponse>(
      `/resources/${id}/draft/import${qs({ draft_revision: draftRevision })}`,
      {
        method: "POST",
        headers: { "Content-Type": file.type || "application/zip" },
        body: file,
      },
    ),
  validateResourceDraft: (id: string) =>
    request<ResourceValidation>(`/resources/${id}/draft/validate`, { method: "POST" }),
  releaseResource: (id: string, body: ReleaseResourceRequest) =>
    request<ReleaseResourceResult>(`/resources/${id}/release`, {
      method: "POST",
      body: JSON.stringify(body),
    }),
  deprecateResourceVersion: (resourceId: string, versionId: string, reason: string) =>
    request<ResourceVersion>(
      `/resources/${resourceId}/versions/${versionId}/deprecate`,
      { method: "POST", body: JSON.stringify({ reason }) },
    ),
  restoreResourceVersionToDraft: (
    resourceId: string,
    versionId: string,
    draftRevision: number,
    confirmDeprecated: boolean,
  ) =>
    request<DraftFileTree>(
      `/resources/${resourceId}/versions/${versionId}/restore-to-draft`,
      {
        method: "POST",
        body: JSON.stringify({
          draft_revision: draftRevision,
          confirm_deprecated: confirmDeprecated,
        }),
      },
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
  resourceInventory: (id: string) =>
    request<ResourceInventoryMonitoring>(`/resources/${id}/inventory`),
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
