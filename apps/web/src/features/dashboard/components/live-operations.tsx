import { Boxes, Cpu, Gauge, Server, Users, Wifi, type LucideIcon } from "lucide-react"

import {
  formatBytePair,
  formatOptionalCount,
  formatOptionalPercent,
  formatThreshold,
  formatTimestamp,
  normalizePercent,
  percentOf,
} from "@/features/dashboard/lib/dashboard-formatters"
import type {
  DashboardSummary,
  ResourceUsageAnalytics,
} from "@/shared/api/client"
import { cn } from "@/shared/lib/utils"
import { Badge } from "@/shared/ui/badge"
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/shared/ui/card"
import { LoadingState, Skeleton } from "@/shared/ui/skeleton"

export function LiveOperations({
  summary,
  analytics,
  loading = false,
  className,
  announceLoading = true,
}: {
  summary: DashboardSummary | undefined
  analytics: ResourceUsageAnalytics | undefined
  loading?: boolean
  className?: string
  announceLoading?: boolean
}) {
  if (loading) {
    return (
      <LiveOperationsSkeleton
        className={className}
        announce={announceLoading}
      />
    )
  }

  const presence = summary?.presence
  const realtime = summary?.realtime
  const host = summary?.host_metrics
  const totals = analytics?.totals
  const memoryPercent = percentOf(host?.memory_used_bytes, host?.memory_total_bytes)
  const vramPercent = percentOf(host?.vram_used_bytes, host?.vram_total_bytes)

  return (
    <Card className={className}>
      <CardHeader>
        <div>
          <CardTitle>Live operations</CardTitle>
          <CardDescription className="mt-0.5">
            Presence, this-node SSE streams and Conductor host sampling.
          </CardDescription>
        </div>
        <Badge tone="accent">Current state</Badge>
      </CardHeader>
      <CardContent className="grid gap-4">
        <section aria-labelledby="dashboard-presence">
          <div className="flex items-center justify-between gap-2">
            <h3 id="dashboard-presence" className="text-xs font-semibold">
              Recent presence
            </h3>
            <Badge>
              {presence
                ? `Last ${formatThreshold(presence.threshold_seconds)}`
                : "Not reported"}
            </Badge>
          </div>
          <div className="mt-2 grid grid-cols-2 gap-2 sm:grid-cols-4 xl:grid-cols-2 2xl:grid-cols-4">
            <LiveDatum
              label="Members seen recently"
              value={formatOptionalCount(presence?.members_seen_recently)}
              icon={Users}
            />
            <LiveDatum
              label="Clients seen recently"
              value={formatOptionalCount(presence?.clients_seen_recently)}
              icon={Server}
            />
            <LiveDatum
              label="SSE streams"
              value={formatOptionalCount(realtime?.active_streams)}
              hint="This node"
              icon={Wifi}
            />
            <LiveDatum
              label="SSE owners"
              value={formatOptionalCount(realtime?.active_owners)}
              hint="This node"
              icon={Users}
            />
          </div>
          <p className="mt-2 text-[0.65rem] text-(--color-text-subtle)">
            {presence
              ? `Presence observed ${formatTimestamp(presence.observed_at)}. SSE scope is this Conductor node.`
              : "Presence is unavailable with the current project snapshot."}
          </p>
        </section>

        <section
          aria-labelledby="dashboard-host-metrics"
          className="border-t border-(--border-soft) pt-4"
        >
          <div className="flex items-center justify-between gap-2">
            <h3 id="dashboard-host-metrics" className="text-xs font-semibold">
              Conductor host
            </h3>
            <Badge>
              {host?.scope === "conductor_host" ? "Host scope" : "Not reported"}
            </Badge>
          </div>
          <div className="mt-3 grid gap-3 sm:grid-cols-2 xl:grid-cols-1 2xl:grid-cols-2">
            <MetricMeter
              label="CPU"
              value={host?.cpu_usage_percent}
              display={formatOptionalPercent(host?.cpu_usage_percent)}
              icon={Cpu}
            />
            <MetricMeter
              label="Memory"
              value={memoryPercent}
              display={formatBytePair(
                host?.memory_used_bytes,
                host?.memory_total_bytes,
              )}
              icon={Server}
            />
            <MetricMeter
              label="GPU"
              value={host?.gpu_usage_percent}
              display={formatOptionalPercent(host?.gpu_usage_percent)}
              icon={Gauge}
            />
            <MetricMeter
              label="VRAM"
              value={vramPercent}
              display={formatBytePair(
                host?.vram_used_bytes,
                host?.vram_total_bytes,
              )}
              icon={Boxes}
            />
          </div>
          <p className="mt-2 text-[0.65rem] text-(--color-text-subtle)">
            {host
              ? `Sampled ${formatTimestamp(host.sampled_at)} · Conductor host, not EvoFlux client hardware.`
              : "Host metrics are unavailable with the current project snapshot."}
          </p>
        </section>

        <section
          aria-labelledby="dashboard-delivery-state"
          className="border-t border-(--border-soft) pt-4"
        >
          <h3 id="dashboard-delivery-state" className="text-xs font-semibold">
            Resource delivery state
          </h3>
          <div className="mt-2 grid grid-cols-3 gap-2">
            <CompactDatum
              label="Installed clients"
              value={formatOptionalCount(totals?.installed_installations)}
              tone="success"
            />
            <CompactDatum
              label="Pending states"
              value={formatOptionalCount(totals?.pending_installations)}
              tone="warning"
            />
            <CompactDatum
              label="Attention states"
              value={formatOptionalCount(totals?.attention_installations)}
              tone="danger"
            />
          </div>
        </section>
      </CardContent>
    </Card>
  )
}

export function LiveOperationsSkeleton({
  className,
  announce = true,
}: {
  className?: string
  announce?: boolean
}) {
  return (
    <Card className={className}>
      <LoadingState label="Loading live operations" announce={announce}>
        <CardHeader>
          <div className="min-w-0 flex-1">
            <Skeleton className="h-4 w-28" />
            <Skeleton className="mt-2 h-3 w-64 max-w-full" />
          </div>
          <Skeleton className="h-6 w-20" />
        </CardHeader>
        <CardContent className="grid gap-4">
          <div>
            <div className="flex items-center justify-between gap-2">
              <Skeleton className="h-3 w-24" />
              <Skeleton className="h-5 w-16" />
            </div>
            <div className="mt-2 grid grid-cols-2 gap-2 sm:grid-cols-4 xl:grid-cols-2 2xl:grid-cols-4">
              {Array.from({ length: 4 }, (_, index) => (
                <div key={index} className="rounded-lg border border-(--border-soft) p-2.5">
                  <Skeleton className="h-3 w-20 max-w-full" />
                  <Skeleton className="mt-2 h-5 w-10" />
                  <Skeleton className="mt-1 h-2.5 w-14" />
                </div>
              ))}
            </div>
          </div>
          <div className="border-t border-(--border-soft) pt-4">
            <div className="flex items-center justify-between gap-2">
              <Skeleton className="h-3 w-24" />
              <Skeleton className="h-5 w-16" />
            </div>
            <div className="mt-3 grid gap-3 sm:grid-cols-2 xl:grid-cols-1 2xl:grid-cols-2">
              {Array.from({ length: 4 }, (_, index) => (
                <div key={index}>
                  <div className="flex items-center gap-2">
                    <Skeleton className="size-3.5" />
                    <Skeleton className="h-3 w-16" />
                    <Skeleton className="ml-auto h-3 w-12" />
                  </div>
                  <Skeleton className="mt-2 h-1.5 w-full rounded-full" />
                </div>
              ))}
            </div>
          </div>
          <div className="border-t border-(--border-soft) pt-4">
            <Skeleton className="h-3 w-32" />
            <div className="mt-2 grid grid-cols-3 gap-2">
              {Array.from({ length: 3 }, (_, index) => (
                <div key={index} className="rounded-lg border border-(--border-soft) p-2">
                  <Skeleton className="mx-auto h-5 w-8" />
                  <Skeleton className="mx-auto mt-1 h-2.5 w-16 max-w-full" />
                </div>
              ))}
            </div>
          </div>
        </CardContent>
      </LoadingState>
    </Card>
  )
}

function LiveDatum({
  label,
  value,
  hint,
  icon: Icon,
}: {
  label: string
  value: string
  hint?: string
  icon: LucideIcon
}) {
  return (
    <div className="rounded-lg border border-(--border-soft) bg-(--bg-key)/25 p-2.5">
      <div className="flex items-center justify-between gap-2 text-(--color-text-subtle)">
        <span className="truncate text-[0.65rem]">{label}</span>
        <Icon className="size-3 shrink-0" />
      </div>
      <div className="mt-1 text-lg font-semibold tabular-nums">{value}</div>
      {hint && (
        <div className="text-[0.62rem] text-(--color-text-subtle)">{hint}</div>
      )}
    </div>
  )
}

function MetricMeter({
  label,
  value,
  display,
  icon: Icon,
}: {
  label: string
  value: number | null | undefined
  display: string
  icon: LucideIcon
}) {
  const normalized = normalizePercent(value)
  return (
    <div>
      <div className="flex items-center gap-2 text-xs">
        <Icon className="size-3.5 text-(--color-text-subtle)" />
        <span className="min-w-0 flex-1 text-(--color-text-muted)">{label}</span>
        <span className="shrink-0 font-medium tabular-nums">{display}</span>
      </div>
      {normalized == null ? (
        <div className="mt-1.5 h-1.5 rounded-full bg-(--bg-key)" />
      ) : (
        <div
          role="progressbar"
          aria-label={`${label} usage`}
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={normalized}
          className="mt-1.5 h-1.5 overflow-hidden rounded-full bg-(--bg-key)"
        >
          <div
            className={cn(
              "h-full rounded-full",
              normalized >= 90
                ? "bg-(--color-error)"
                : normalized >= 75
                  ? "bg-(--color-warning)"
                  : "bg-(--color-accent)",
            )}
            style={{ width: `${normalized}%` }}
          />
        </div>
      )}
    </div>
  )
}

function CompactDatum({
  label,
  value,
  tone,
}: {
  label: string
  value: string
  tone: "success" | "warning" | "danger"
}) {
  return (
    <div className="rounded-lg border border-(--border-soft) px-2 py-2 text-center">
      <div
        className={cn(
          "text-lg font-semibold tabular-nums",
          tone === "success" && "text-(--color-success)",
          tone === "warning" && "text-(--color-warning)",
          tone === "danger" && "text-(--color-error)",
        )}
      >
        {value}
      </div>
      <div className="text-[0.62rem] leading-tight text-(--color-text-subtle)">
        {label}
      </div>
    </div>
  )
}
