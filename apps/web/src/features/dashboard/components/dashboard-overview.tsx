import {
  AlertTriangle,
  ArrowRight,
  CheckCircle2,
  CircleDollarSign,
  Clock3,
  Gauge,
  Wifi,
} from "lucide-react"

import type { DashboardAttentionItem } from "@/features/dashboard/lib/dashboard-model"
import {
  dashboardAttentionTone,
  formatOptionalCount,
} from "@/features/dashboard/lib/dashboard-formatters"
import { formatDuration } from "@/features/members/components/usage-formatters"
import { formatEstimatedCost } from "@/features/resource-usage/components/resource-usage-formatters"
import type {
  DashboardSummary,
  ResourceUsageAnalytics,
} from "@/shared/api/client"
import { StatCard, StatCardSkeleton } from "@/shared/components/stat-card"
import { terminalRequestSuccessRate } from "@/shared/lib/telemetry-metrics"
import { StatusDot } from "@/shared/ui/badge"

export function DashboardMetricGrid({
  summary,
  analytics,
  summaryLoading = false,
  analyticsLoading = false,
}: {
  summary: DashboardSummary | undefined
  analytics: ResourceUsageAnalytics | undefined
  summaryLoading?: boolean
  analyticsLoading?: boolean
}) {
  const totals = analytics?.totals
  const successRate = terminalRequestSuccessRate(
    totals?.successes,
    totals?.requests,
  )
  const activeStreams = summary?.realtime?.active_streams
  const activeOwners = summary?.realtime?.active_owners
  const attention = totals?.attention_installations ?? 0
  const pending = totals?.pending_installations ?? 0

  return (
    <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3 2xl:grid-cols-6">
      {summaryLoading ? (
        <StatCardSkeleton />
      ) : (
        <StatCard
          label="SSE streams · this node"
          value={formatOptionalCount(activeStreams)}
          hint={
            activeOwners == null
              ? "Realtime state not reported"
              : `${activeOwners.toLocaleString()} active owners`
          }
          icon={Wifi}
          tone={(activeStreams ?? 0) > 0 ? "accent" : "neutral"}
        />
      )}
      {analyticsLoading ? (
        Array.from({ length: 5 }, (_, index) => <StatCardSkeleton key={index} />)
      ) : (
        <>
          <StatCard
            label="Governed requests"
            value={analytics ? (totals?.requests ?? 0).toLocaleString() : "—"}
            hint={
              analytics
                ? `${(totals?.resource_uses ?? 0).toLocaleString()} attributed resource uses`
                : "Selected-range analytics unavailable"
            }
            icon={Gauge}
            tone={analytics ? "accent" : "neutral"}
          />
          <StatCard
            label="Success rate"
            value={analytics && successRate != null ? `${successRate}%` : "—"}
            hint={
              analytics
                ? `${totals?.errors ?? 0} errors · ${totals?.blocked ?? 0} blocked`
                : "Selected-range analytics unavailable"
            }
            icon={CheckCircle2}
            tone={
              successRate == null
                ? "neutral"
                : successRate >= 90
                  ? "success"
                  : "warning"
            }
          />
          <StatCard
            label="Average request duration"
            value={
              analytics && (totals?.requests ?? 0) > 0
                ? formatDuration(totals?.average_duration_ms ?? 0)
                : "—"
            }
            hint={
              analytics && (totals?.requests ?? 0) === 0
                ? "No terminal requests in this range"
                : analytics
                  ? "Terminal governed requests"
                  : "Selected-range analytics unavailable"
            }
            icon={Clock3}
          />
          <StatCard
            label="Estimated cost"
            value={
              analytics
                ? formatEstimatedCost(totals?.estimated_cost_usd_micros ?? 0)
                : "—"
            }
            hint={
              analytics
                ? `${totals?.unpriced_model_calls ?? 0} unpriced model calls`
                : "Selected-range analytics unavailable"
            }
            icon={CircleDollarSign}
            tone={(totals?.unpriced_model_calls ?? 0) > 0 ? "warning" : "neutral"}
          />
          <StatCard
            label="Delivery attention"
            value={analytics ? attention.toLocaleString() : "—"}
            hint={
              analytics
                ? `${pending.toLocaleString()} pending resource states · ${totals?.installed_installations ?? 0} installed clients`
                : "Current delivery state unavailable"
            }
            icon={AlertTriangle}
            tone={!analytics ? "neutral" : attention > 0 ? "warning" : "success"}
          />
        </>
      )}
    </div>
  )
}

export function DashboardAttentionRail({
  items,
  overviewHref,
}: {
  items: DashboardAttentionItem[]
  overviewHref: (filters?: Record<string, string>) => string
}) {
  return (
    <section
      aria-labelledby="dashboard-attention-title"
      className="rounded-xl border border-(--color-warning)/30 bg-(--color-warning)/6"
    >
      <div className="flex flex-col gap-3 px-4 py-3 lg:flex-row lg:items-start">
        <div className="flex shrink-0 items-center gap-2 lg:w-40">
          <span className="grid size-7 place-items-center rounded-md bg-(--color-warning)/12 text-(--color-warning)">
            <AlertTriangle className="size-3.5" />
          </span>
          <div>
            <h2 id="dashboard-attention-title" className="text-sm font-medium">
              Needs attention
            </h2>
            <p className="text-[0.68rem] text-(--color-text-subtle)">
              {items.length} reported conditions
            </p>
          </div>
        </div>
        <div className="grid min-w-0 flex-1 gap-2 md:grid-cols-2 xl:grid-cols-3">
          {items.map((item) => {
            const href =
              item.id === "members"
                ? "/app/members"
                : overviewHref(item.filter)
            return (
              <a
                key={item.id}
                href={href}
                className="group flex min-w-0 items-start gap-2 rounded-lg border border-(--border-soft) bg-(--bg-card)/75 px-3 py-2 outline-none transition-colors hover:border-(--color-border-strong) focus-visible:ring-2 focus-visible:ring-(--focus-ring)/35"
              >
                <StatusDot
                  tone={dashboardAttentionTone(item.tone)}
                  className="mt-1.5"
                />
                <span className="min-w-0 flex-1">
                  <span className="block text-xs font-medium text-(--color-text)">
                    {item.label}
                  </span>
                  <span className="mt-0.5 block text-[0.68rem] leading-snug text-(--color-text-subtle)">
                    {item.detail}
                  </span>
                </span>
                <ArrowRight className="mt-1 size-3.5 shrink-0 text-(--color-text-subtle) transition-transform group-hover:translate-x-0.5" />
              </a>
            )
          })}
        </div>
      </div>
    </section>
  )
}
