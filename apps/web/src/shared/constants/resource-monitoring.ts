import { RESOURCE_KIND, type ResourceKind } from "@/shared/constants/resource"

export const RESOURCE_INVENTORY_STATE = {
  PENDING: "pending",
  STAGED: "staged",
  TRUST_PENDING: "trust_pending",
  UPDATE_PENDING: "update_pending",
  APPLIED: "applied",
  IN_SYNC: "in_sync",
  DECLINED: "declined",
  INCOMPATIBLE: "incompatible",
  OWNERSHIP_CONFLICT: "ownership_conflict",
  PROJECT_SCOPE_MISMATCH: "project_scope_mismatch",
  ERROR: "error",
  REMOVED: "removed",
} as const

export const RESOURCE_INSTALLED_STATES = [
  RESOURCE_INVENTORY_STATE.APPLIED,
  RESOURCE_INVENTORY_STATE.IN_SYNC,
] as const

export const RESOURCE_PENDING_STATES = [
  RESOURCE_INVENTORY_STATE.PENDING,
  RESOURCE_INVENTORY_STATE.STAGED,
  RESOURCE_INVENTORY_STATE.TRUST_PENDING,
  RESOURCE_INVENTORY_STATE.UPDATE_PENDING,
] as const

export const RESOURCE_KIND_USAGE_PATHS = {
  [RESOURCE_KIND.PLUGIN]: {
    overview: "/app/resources/plugins",
    activity: "/app/resources/plugins/activity",
    usage: "/app/resources/plugins/usage",
  },
  [RESOURCE_KIND.SKILL]: {
    overview: "/app/resources/skills",
    activity: "/app/resources/skills/activity",
    usage: "/app/resources/skills/usage",
  },
  [RESOURCE_KIND.AGENT]: {
    overview: "/app/resources/agents",
    activity: "/app/resources/agents/activity",
    usage: "/app/resources/agents/usage",
  },
} as const satisfies Record<
  Extract<ResourceKind, "plugin" | "skill" | "agent">,
  { overview: string; activity: string; usage: string }
>

export const RESOURCE_KIND_USAGE_ROUTE_PATHS = {
  [RESOURCE_KIND.PLUGIN]: {
    activity: "/resources/plugins/activity",
    usage: "/resources/plugins/usage",
  },
  [RESOURCE_KIND.SKILL]: {
    activity: "/resources/skills/activity",
    usage: "/resources/skills/usage",
  },
  [RESOURCE_KIND.AGENT]: {
    activity: "/resources/agents/activity",
    usage: "/resources/agents/usage",
  },
} as const

export const RESOURCE_MONITORING_TAB = {
  OVERVIEW: "overview",
  INSTALLATIONS: "installations",
  ACTIVITY: "activity",
  USAGE: "usage",
} as const

export type ResourceMonitoringTab =
  (typeof RESOURCE_MONITORING_TAB)[keyof typeof RESOURCE_MONITORING_TAB]
