import type {
  DashboardSummary,
  ResourceUsageAnalytics,
} from "@/shared/api/client"
import type { UsageRangePreset } from "@/shared/constants/telemetry"

export const DASHBOARD_QUERY_KEYS = {
  summary: ["dashboard", "summary"] as const,
  analytics: (
    role: string | undefined,
    from: string | undefined,
    to: string | undefined,
  ) => ["dashboard", "analytics", role, from, to] as const,
} as const

export type DashboardAttentionTone = "danger" | "warning" | "info"

export interface DashboardAttentionItem {
  id: "delivery" | "errors" | "blocked" | "unpriced" | "members"
  tone: DashboardAttentionTone
  label: string
  detail: string
  filter?: Record<string, string>
}

export function dashboardResourceTotal(summary: DashboardSummary | undefined) {
  if (!summary) return 0
  return (
    summary.resources.agents +
    summary.resources.skills +
    summary.resources.plugins +
    summary.resources.workflows
  )
}

export function hasDashboardTelemetry(data: ResourceUsageAnalytics | undefined) {
  if (!data) return false
  return (
    data.totals.requests > 0 ||
    data.totals.model_calls > 0 ||
    data.totals.tool_calls > 0 ||
    data.totals.resource_uses > 0
  )
}

export function buildDashboardAttention(
  analytics: ResourceUsageAnalytics | undefined,
  pendingMembers = 0,
) {
  const totals = analytics?.totals
  const items: DashboardAttentionItem[] = []

  if ((totals?.attention_installations ?? 0) > 0) {
    items.push({
      id: "delivery",
      tone: "danger",
      label: `${formatCount(totals?.attention_installations ?? 0)} resource states need attention`,
      detail: "Review desired and observed delivery state.",
    })
  }
  if ((totals?.errors ?? 0) > 0) {
    items.push({
      id: "errors",
      tone: "danger",
      label: `${formatCount(totals?.errors ?? 0)} governed requests failed`,
      detail: "Inspect sanitized request outcomes in Analytics.",
      filter: { status: "error" },
    })
  }
  if ((totals?.blocked ?? 0) > 0) {
    items.push({
      id: "blocked",
      tone: "warning",
      label: `${formatCount(totals?.blocked ?? 0)} governed requests were blocked`,
      detail: "Review policy and tool outcomes for this range.",
      filter: { status: "blocked" },
    })
  }
  if ((totals?.unpriced_model_calls ?? 0) > 0) {
    items.push({
      id: "unpriced",
      tone: "warning",
      label: `${formatCount(totals?.unpriced_model_calls ?? 0)} model calls are unpriced`,
      detail: "Estimated cost excludes calls without a catalog estimate.",
    })
  }
  if (pendingMembers > 0) {
    items.push({
      id: "members",
      tone: "info",
      label: `${formatCount(pendingMembers)} member ${pendingMembers === 1 ? "request" : "requests"} pending`,
      detail: "Administrator approval is required.",
    })
  }

  return items
}

export function dashboardAnalyticsHref(
  path: string,
  preset: UsageRangePreset,
  customFrom: string,
  customTo: string,
  filters: Record<string, string> = {},
) {
  const search = new URLSearchParams()
  if (preset !== "month") search.set("range", preset)
  if (preset === "custom") {
    search.set("from", customFrom)
    search.set("to", customTo)
  }
  for (const [key, value] of Object.entries(filters)) {
    if (value) search.set(key, value)
  }
  const suffix = search.toString()
  return suffix ? `${path}?${suffix}` : path
}

export function dashboardUpdatedAt(...timestamps: number[]) {
  const value = Math.max(0, ...timestamps)
  return value > 0 ? value : null
}

function formatCount(value: number) {
  return value.toLocaleString()
}
