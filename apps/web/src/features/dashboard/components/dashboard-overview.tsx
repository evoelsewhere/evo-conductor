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
  formatBytePair,
  formatOptionalCount,
  formatOptionalPercent,
  formatTimestamp,
} from "@/features/dashboard/lib/dashboard-formatters"
import { formatDuration } from "@/features/members/components/usage-formatters"
import { formatEstimatedCost } from "@/features/resource-usage/components/resource-usage-formatters"
import type {
  DashboardSummary,
  ResourceUsageAnalytics,
  ResourceUsageScope,
} from "@/shared/api/client"
import { cn } from "@/shared/lib/utils"
import {
  requestAttributionCoverage,
  terminalRequestSuccessRate,
} from "@/shared/lib/telemetry-metrics"
import { StatusDot } from "@/shared/ui/badge"
import { Card } from "@/shared/ui/card"
import { LoadingState, Skeleton } from "@/shared/ui/skeleton"

export function DashboardMetricGrid({
  summary,
  analytics,
  scope,
  summaryLoading = false,
  analyticsLoading = false,
}: {
  summary: DashboardSummary | undefined
  analytics: ResourceUsageAnalytics | undefined
  scope: ResourceUsageScope
  summaryLoading?: boolean
  analyticsLoading?: boolean
}) {
  const totals = analytics?.totals
  const successRate = terminalRequestSuccessRate(
    totals?.successes,
    totals?.requests,
  )
  const attributionCoverage = requestAttributionCoverage(
    totals?.governed_requests,
    totals?.all_requests,
  )
  const activeStreams = summary?.realtime?.active_streams
  const activeOwners = summary?.realtime?.active_owners
  const attention = totals?.attention_installations ?? 0
  const pending = totals?.pending_installations ?? 0

  const metrics = [
    {
      label: "SSE streams · this node",
      value: formatOptionalCount(activeStreams),
      hint:
        activeOwners == null
          ? "Realtime not reported"
          : `${activeOwners.toLocaleString()} active owners`,
      icon: Wifi,
      tone: (activeStreams ?? 0) > 0 ? "accent" : "neutral",
      loading: summaryLoading,
    },
    {
      label: scope === "all" ? "EvoFlux requests" : "Governed requests",
      value: analytics ? (totals?.requests ?? 0).toLocaleString() : "—",
      hint: analytics
        ? scope === "all"
          ? `${(totals?.governed_requests ?? 0).toLocaleString()} governed · ${attributionCoverage ?? 0}% coverage`
          : `${(totals?.all_requests ?? 0).toLocaleString()} received · ${attributionCoverage ?? 0}% coverage`
        : "Selected range unavailable",
      icon: Gauge,
      tone: analytics ? "accent" : "neutral",
      loading: analyticsLoading,
    },
    {
      label: "Success rate",
      value: analytics && successRate != null ? `${successRate}%` : "—",
      hint: analytics
        ? `${totals?.errors ?? 0} errors · ${totals?.blocked ?? 0} blocked`
        : "Selected range unavailable",
      icon: CheckCircle2,
      tone:
        successRate == null
          ? "neutral"
          : successRate >= 90
            ? "success"
            : "warning",
      loading: analyticsLoading,
    },
    {
      label: "Average duration",
      value:
        analytics && (totals?.requests ?? 0) > 0
          ? formatDuration(totals?.average_duration_ms ?? 0)
          : "—",
      hint:
        analytics && (totals?.requests ?? 0) === 0
          ? "No terminal requests"
          : analytics
            ? "Terminal requests"
            : "Selected range unavailable",
      icon: Clock3,
      tone: "neutral",
      loading: analyticsLoading,
    },
    {
      label: "Estimated cost",
      value: analytics
        ? formatEstimatedCost(totals?.estimated_cost_usd_micros ?? 0)
        : "—",
      hint: analytics
        ? `${scope === "all" ? "All received" : "Governed"} · ${totals?.unpriced_model_calls ?? 0} unpriced calls`
        : "Selected range unavailable",
      icon: CircleDollarSign,
      tone: (totals?.unpriced_model_calls ?? 0) > 0 ? "warning" : "neutral",
      loading: analyticsLoading,
    },
    {
      label: "Delivery attention",
      value: analytics ? attention.toLocaleString() : "—",
      hint: analytics
        ? `${pending.toLocaleString()} pending · ${totals?.installed_installations ?? 0} installed clients`
        : "Current state unavailable",
      icon: AlertTriangle,
      tone: !analytics ? "neutral" : attention > 0 ? "warning" : "success",
      loading: analyticsLoading,
    },
  ] as const

  return (
    <Card role="region" aria-label="Operational summary" className="overflow-hidden">
      <dl className="grid sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-6">
        {metrics.map((metric, index) => (
          <MetricStripItem key={metric.label} {...metric} index={index} />
        ))}
      </dl>
      {summaryLoading ? (
        <div className="border-t border-(--border-soft) px-4 py-2.5">
          <Skeleton className="h-3 w-4/5 max-w-xl" />
        </div>
      ) : (
        <HostRuntimeStrip summary={summary} />
      )}
    </Card>
  )
}

function HostRuntimeStrip({ summary }: { summary: DashboardSummary | undefined }) {
  const host = summary?.host_metrics
  return (
    <div className="flex flex-wrap items-center gap-x-4 gap-y-1 border-t border-(--border-soft) px-4 py-2 text-[0.68rem]">
      <span className="font-medium text-(--color-text-muted)">Conductor host</span>
      <HostDatum label="CPU" value={formatOptionalPercent(host?.cpu_usage_percent)} />
      <HostDatum
        label="Memory"
        value={formatBytePair(host?.memory_used_bytes, host?.memory_total_bytes)}
      />
      <HostDatum label="GPU" value={formatOptionalPercent(host?.gpu_usage_percent)} />
      <HostDatum
        label="VRAM"
        value={formatBytePair(host?.vram_used_bytes, host?.vram_total_bytes)}
      />
      <span className="ml-auto text-(--color-text-subtle)">
        {host
          ? `Sampled ${formatTimestamp(host.sampled_at)} · host scope`
          : "Host sampling unavailable"}
      </span>
    </div>
  )
}

function HostDatum({ label, value }: { label: string; value: string }) {
  return (
    <span
      data-dashboard-host-metric={label}
      className="whitespace-nowrap text-(--color-text-subtle)"
    >
      {label} <strong className="font-medium text-(--color-text)">{value}</strong>
    </span>
  )
}

function MetricStripItem({
  label,
  value,
  hint,
  icon: Icon,
  tone,
  loading,
  index,
}: {
  label: string
  value: string
  hint: string
  icon: typeof Wifi
  tone: "neutral" | "accent" | "success" | "warning"
  loading: boolean
  index: number
}) {
  return (
    <div
      data-dashboard-metric={label}
      className={cn(
        "min-w-0 border-(--border-soft) px-4 py-3",
        index < 5 && "border-b xl:border-b-0 xl:border-r",
        index === 1 && "sm:border-b lg:border-r",
        index === 2 && "lg:border-r",
        index === 3 && "sm:border-r xl:border-r",
        index === 4 && "lg:border-r xl:border-r",
      )}
    >
      {loading ? (
        <LoadingState label={`Loading ${label}`} announce={false}>
          <Skeleton className="h-3 w-24" />
          <Skeleton className="mt-2 h-7 w-20" />
          <Skeleton className="mt-1.5 h-3 w-32 max-w-full" />
        </LoadingState>
      ) : (
        <>
          <dt className="flex items-center gap-2 text-[0.68rem] font-medium text-(--color-text-muted)">
            <Icon
              className={cn(
                "size-3.5",
                tone === "accent" && "text-(--color-accent)",
                tone === "success" && "text-(--color-success)",
                tone === "warning" && "text-(--color-warning)",
              )}
            />
            <span className="truncate">{label}</span>
          </dt>
          <dd className="mt-1 text-2xl font-semibold tracking-tight tabular-nums">
            {value}
          </dd>
          <dd className="mt-0.5 truncate text-[0.65rem] text-(--color-text-subtle)">
            {hint}
          </dd>
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
        <div className="min-w-0 flex-1 divide-y divide-(--border-soft) lg:grid lg:grid-cols-2 lg:divide-y-0 xl:grid-cols-3">
          {items.map((item) => {
            const href =
              item.id === "members"
                ? "/app/members"
                : overviewHref(item.filter)
            return (
              <a
                key={item.id}
                href={href}
                className="group flex min-w-0 items-start gap-2 px-3 py-2 outline-none transition-colors hover:bg-(--bg-key)/45 focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-(--focus-ring)/35"
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
