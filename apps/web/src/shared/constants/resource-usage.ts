import { PRIMARY_ROLE } from "@/shared/constants/member"
import { RESOURCE_KIND } from "@/shared/constants/resource"
import { TelemetryEventStatus } from "@/shared/constants/telemetry"

export const RESOURCE_USAGE_PAGE_SIZE = 50
export const RESOURCE_USAGE_OVERVIEW_ACTIVITY_LIMIT = 8
export const RESOURCE_USAGE_ALL_FILTER = "all"
export const RESOURCE_USAGE_PERCENT_SCALE = 100
export const USD_MICROS = 1_000_000

export const RESOURCE_USAGE_VIEW = {
  OVERVIEW: "overview",
  ACTIVITY: "activity",
  USAGE: "usage",
} as const

export type ResourceUsageView =
  (typeof RESOURCE_USAGE_VIEW)[keyof typeof RESOURCE_USAGE_VIEW]

export const RESOURCE_USAGE_PATHS = {
  overview: "/app/resources/usage",
  activity: "/app/resources/usage/activity",
  usage: "/app/resources/usage/usage",
} as const

export const RESOURCE_USAGE_ROUTE_PATHS = {
  overview: "/resources/usage",
  activity: "/resources/usage/activity",
  usage: "/resources/usage/usage",
  requestDetail: "/resources/usage/activity/$userId/$requestId",
  legacy: "/analytics/resource-usage",
} as const

export const RESOURCE_USAGE_NAV_ITEMS = [
  {
    view: RESOURCE_USAGE_VIEW.OVERVIEW,
    label: "Overview",
    to: RESOURCE_USAGE_PATHS.overview,
  },
  {
    view: RESOURCE_USAGE_VIEW.ACTIVITY,
    label: "Activity",
    to: RESOURCE_USAGE_PATHS.activity,
  },
  {
    view: RESOURCE_USAGE_VIEW.USAGE,
    label: "Usage",
    to: RESOURCE_USAGE_PATHS.usage,
  },
] as const

export const RESOURCE_USAGE_COST_SOURCE_LABELS = {
  evoflux_catalog: "EvoFlux catalog estimate",
  unpriced: "Unpriced",
} as const

export const RESOURCE_USAGE_ROLE_OPTIONS = [
  { value: RESOURCE_USAGE_ALL_FILTER, label: "All roles" },
  { value: PRIMARY_ROLE.ADMIN, label: "Admin" },
  { value: PRIMARY_ROLE.CONTRIBUTE, label: "Contribute" },
  { value: PRIMARY_ROLE.USER, label: "User" },
] as const

export const RESOURCE_USAGE_KIND_OPTIONS = [
  { value: RESOURCE_USAGE_ALL_FILTER, label: "All resources" },
  { value: RESOURCE_KIND.AGENT, label: "Agents" },
  { value: RESOURCE_KIND.SKILL, label: "Skills" },
  { value: RESOURCE_KIND.PLUGIN, label: "Plugins" },
] as const

export const RESOURCE_USAGE_STATUS_OPTIONS = [
  { value: RESOURCE_USAGE_ALL_FILTER, label: "Any outcome" },
  { value: TelemetryEventStatus.Success, label: "Success" },
  { value: TelemetryEventStatus.Error, label: "Error" },
  { value: TelemetryEventStatus.Blocked, label: "Blocked" },
  { value: TelemetryEventStatus.Cancelled, label: "Cancelled" },
] as const

export const RESOURCE_USAGE_RELATION_OPTIONS = [
  { value: RESOURCE_USAGE_ALL_FILTER, label: "Any relation" },
  { value: "executing_agent", label: "Executing Agent" },
  { value: "activated_skill", label: "Activated Skill" },
  { value: "plugin_contributed_skill", label: "Plugin Skill" },
  { value: "plugin_contributed_tool", label: "Plugin Tool" },
] as const

export const RESOURCE_USAGE_QUERY_KEY = "resource-usage-analytics"
export const RESOURCE_USAGE_MEMBERS_QUERY_KEY = "resource-usage-members"
export const RESOURCE_USAGE_RESOURCES_QUERY_KEY = "resource-usage-resources"
export const RESOURCE_USAGE_VERSIONS_QUERY_KEY = "resource-usage-versions"
export const RESOURCE_USAGE_INSTALLATIONS_QUERY_KEY = "resource-usage-installations"
