import { Server, Users, Wifi, type LucideIcon } from "lucide-react"

import {
  formatOptionalCount,
  formatThreshold,
  formatTimestamp,
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
  const totals = analytics?.totals

  return (
    <Card className={className}>
      <CardHeader>
        <div>
          <CardTitle>Live operations</CardTitle>
          <CardDescription className="mt-0.5">
            Recent member presence, this-node streams and delivery state.
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
          <dl className="mt-2 grid grid-cols-2 overflow-hidden rounded-lg border border-(--border-soft) sm:grid-cols-4 xl:grid-cols-2 2xl:grid-cols-4">
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
          </dl>
          <p className="mt-2 text-[0.65rem] text-(--color-text-subtle)">
            {presence
              ? `Presence observed ${formatTimestamp(presence.observed_at)}. SSE scope is this Conductor node.`
              : "Presence is unavailable with the current project snapshot."}
          </p>
        </section>

        <section
          aria-labelledby="dashboard-delivery-state"
          className="border-t border-(--border-soft) pt-4"
        >
          <h3 id="dashboard-delivery-state" className="text-xs font-semibold">
            Resource delivery state
          </h3>
          <dl className="mt-2 grid grid-cols-3 divide-x divide-(--border-soft) border-y border-(--border-soft)">
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
          </dl>
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
    <div className="border-r border-b border-(--border-soft) px-3 py-2.5 last:border-r-0 sm:border-b-0 xl:border-b 2xl:border-b-0">
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
    <div className="px-2 py-2 text-center">
      <dd
        className={cn(
          "text-lg font-semibold tabular-nums",
          tone === "success" && "text-(--color-success)",
          tone === "warning" && "text-(--color-warning)",
          tone === "danger" && "text-(--color-error)",
        )}
      >
        {value}
      </dd>
      <dt className="text-[0.62rem] leading-tight text-(--color-text-subtle)">
        {label}
      </dt>
    </div>
  )
}
