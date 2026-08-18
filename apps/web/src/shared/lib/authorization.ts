import type {
  AuthorizationConstraint,
  AuthorizationLifecycle,
  AuthorizationProjection,
  PermissionKey,
} from "@/shared/api/client"
import type { ResourceKind } from "@/shared/constants/resource"

export const PERMISSION = {
  AUTHORIZATION_READ_SELF: "authorization.grants.read_self",
  SESSION_READ_SELF: "session.self.read",
  SESSION_PASSWORD_CHANGE: "session.password.change",
  PROJECT_BRANDING_READ: "project.branding.read",
  PROJECT_DASHBOARD_READ: "project.dashboard.read",
  PROJECT_SETTINGS_READ: "project.settings.read",
  PROJECT_SETTINGS_MANAGE: "project.settings.manage",
  MEMBER_DIRECTORY_READ: "member.directory.read",
  MEMBER_MANAGE: "member.manage",
  MEMBER_PRIVATE_READ_SELF: "member.private.read_self",
  MEMBER_PRIVATE_READ_ANY: "member.private.read_any",
  TELEMETRY_PROJECT_READ: "telemetry.project.read",
  TELEMETRY_MEMBER_READ_SELF: "telemetry.member.read_self",
  TELEMETRY_MEMBER_READ_ANY: "telemetry.member.read_any",
  TAXONOMY_READ: "taxonomy.read",
  TAXONOMY_DEFINITION_MANAGE: "taxonomy.definition.manage",
  MEMBER_TAG_ASSIGNMENT_MANAGE: "member.tag_assignment.manage",
  RESOURCE_CONSUME: "resource.consume",
  RESOURCE_AUTHOR: "resource.author",
  RESOURCE_ACCESS_MANAGE: "resource.access.manage",
  RESOURCE_LIFECYCLE_MANAGE: "resource.lifecycle.manage",
  RESOURCE_RELEASE_NON_EXECUTABLE: "resource.release.non_executable",
  RESOURCE_RELEASE_RESTRICTED: "resource.release.restricted",
  RESOURCE_MONITORING_AGGREGATE_READ: "resource.monitoring.aggregate.read",
  RESOURCE_MONITORING_MEMBER_DETAIL_READ: "resource.monitoring.member_detail.read",
  RESOURCE_FEEDBACK_SUBMIT: "resource.feedback.submit",
  RESOURCE_FEEDBACK_READ: "resource.feedback.read",
  ANALYTICS_VIEW_READ: "analytics_view.read",
  ANALYTICS_VIEW_MANAGE_SELF: "analytics_view.manage_self",
  ANALYTICS_VIEW_MANAGE_ANY: "analytics_view.manage_any",
  CONNECTION_TOKEN_ISSUE_SELF: "connection_token.issue_self",
  CONNECTION_TOKEN_READ_SELF: "connection_token.read_self",
  CONNECTION_TOKEN_REVOKE_SELF: "connection_token.revoke_self",
  CONNECTION_TOKEN_READ_ANY: "connection_token.read_any",
  CONNECTION_TOKEN_REVOKE_ANY: "connection_token.revoke_any",
  AUDIT_READ: "audit.read",
  AUDIT_EXPORT: "audit.export",
} as const satisfies Record<string, PermissionKey>

export const AUTHORIZATION_DECISION = {
  ALLOW: "allow",
  DENY: "deny",
  SERVER_CHECK_REQUIRED: "server_check_required",
} as const

export type AuthorizationDecision =
  (typeof AUTHORIZATION_DECISION)[keyof typeof AUTHORIZATION_DECISION]

export interface AuthorizationTargetContext {
  actorId?: string | null
  targetId?: string | null
  ownerId?: string | null
  resourceKind?: ResourceKind | null
  lifecycle?: AuthorizationLifecycle | null
}

export function evaluatePermission(
  projection: AuthorizationProjection | null,
  permission: PermissionKey,
  target: AuthorizationTargetContext = {},
): AuthorizationDecision {
  const grant = projection?.current_grants.find(
    (candidate) => candidate.permission === permission,
  )
  if (!grant) return AUTHORIZATION_DECISION.DENY
  return evaluateConstraint(grant.constraints, target)
}

export function evaluateConstraint(
  constraint: AuthorizationConstraint,
  target: AuthorizationTargetContext,
): AuthorizationDecision {
  switch (constraint.kind) {
    case "any":
      return AUTHORIZATION_DECISION.ALLOW
    case "self":
      return target.actorId && target.targetId && target.actorId === target.targetId
        ? AUTHORIZATION_DECISION.ALLOW
        : AUTHORIZATION_DECISION.DENY
    case "owner":
      return target.actorId && target.ownerId && target.actorId === target.ownerId
        ? AUTHORIZATION_DECISION.ALLOW
        : AUTHORIZATION_DECISION.DENY
    case "resource_kind_in":
      return target.resourceKind && constraint.values.includes(target.resourceKind)
        ? AUTHORIZATION_DECISION.ALLOW
        : AUTHORIZATION_DECISION.DENY
    case "lifecycle_in":
      return target.lifecycle && constraint.values.includes(target.lifecycle)
        ? AUTHORIZATION_DECISION.ALLOW
        : AUTHORIZATION_DECISION.DENY
    case "same_project":
    case "effective_audience":
      return AUTHORIZATION_DECISION.SERVER_CHECK_REQUIRED
    case "all_of":
      return everyDecision(constraint.items.map((item) => evaluateConstraint(item, target)))
    case "any_of":
      return someDecision(constraint.items.map((item) => evaluateConstraint(item, target)))
  }
}

export function bestAuthorizationDecision(
  decisions: AuthorizationDecision[],
): AuthorizationDecision {
  return someDecision(decisions)
}

export function mayRequest(decision: AuthorizationDecision): boolean {
  return decision !== AUTHORIZATION_DECISION.DENY
}

export function permissionLabel(permission: PermissionKey): string {
  return permission
    .split(".")
    .map((part) => part.replaceAll("_", " "))
    .join(" · ")
}

export function constraintLabel(constraint: AuthorizationConstraint): string {
  switch (constraint.kind) {
    case "any":
      return "Any project target"
    case "self":
      return "Self"
    case "owner":
      return "Owned target"
    case "effective_audience":
      return "Server-verified audience"
    case "same_project":
      return "Server-verified project"
    case "resource_kind_in":
      return constraint.values.map(titleCase).join(" or ")
    case "lifecycle_in":
      return constraint.values.map(titleCase).join(" or ")
    case "all_of":
      return constraint.items.map(constraintLabel).join(" and ")
    case "any_of":
      return constraint.items.map(constraintLabel).join(" or ")
  }
}

function everyDecision(decisions: AuthorizationDecision[]): AuthorizationDecision {
  if (decisions.some((decision) => decision === AUTHORIZATION_DECISION.DENY)) {
    return AUTHORIZATION_DECISION.DENY
  }
  if (
    decisions.some(
      (decision) => decision === AUTHORIZATION_DECISION.SERVER_CHECK_REQUIRED,
    )
  ) {
    return AUTHORIZATION_DECISION.SERVER_CHECK_REQUIRED
  }
  return AUTHORIZATION_DECISION.ALLOW
}

function someDecision(decisions: AuthorizationDecision[]): AuthorizationDecision {
  if (decisions.some((decision) => decision === AUTHORIZATION_DECISION.ALLOW)) {
    return AUTHORIZATION_DECISION.ALLOW
  }
  if (
    decisions.some(
      (decision) => decision === AUTHORIZATION_DECISION.SERVER_CHECK_REQUIRED,
    )
  ) {
    return AUTHORIZATION_DECISION.SERVER_CHECK_REQUIRED
  }
  return AUTHORIZATION_DECISION.DENY
}

function titleCase(value: string): string {
  const label = value.replaceAll("_", " ")
  return label.charAt(0).toUpperCase() + label.slice(1)
}
