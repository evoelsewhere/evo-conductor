import { useQuery } from "@tanstack/react-query"
import { Activity, Bot, Boxes, CircleDollarSign, Download, Gauge, Users, Wrench } from "lucide-react"
import { useState } from "react"

import { DateRangeFilter, useUsageRange } from "@/features/members/components/date-range-filter"
import { formatDuration, formatTokens } from "@/features/members/components/usage-formatters"
import { ResourceUsageActivityTable } from "@/features/resource-usage/components/resource-usage-activity-table"
import {
  ResourceAnalyticsStudio,
  TelemetryReadiness,
  hasAnalyticsData,
} from "@/features/resource-usage/components/resource-analytics-studio"
import {
  ResourceMemberBreakdownTable,
  ResourceModelBreakdownTable,
  ResourceRoleBreakdownTable,
  ResourceToolBreakdownTable,
} from "@/features/resource-usage/components/resource-usage-breakdown-tables"
import {
  RequestOutcomeChart,
  TokenCostChart,
} from "@/features/resource-usage/components/resource-usage-charts"
import { formatEstimatedCost } from "@/features/resource-usage/components/resource-usage-formatters"
import { api, type AnalyticsQuery, type ManagedResource, type ResourceInventoryMonitoring, type ResourceUsageAnalytics } from "@/shared/api/client"
import {
  RESOURCE_INSTALLED_STATES,
  RESOURCE_MONITORING_TAB,
  RESOURCE_PENDING_STATES,
  type ResourceMonitoringTab,
} from "@/shared/constants/resource-monitoring"
import { PRIMARY_ROLE_LABELS } from "@/shared/constants/member"
import { Badge } from "@/shared/ui/badge"
import { Card, CardContent, CardHeader, CardTitle } from "@/shared/ui/card"
import { EmptyState, ErrorState } from "@/shared/ui/empty-state"
import { SkeletonRows } from "@/shared/ui/skeleton"
import { Table, TableBody, TableHead, TableRow, TableTd, TableTh, TableWrap } from "@/shared/ui/table"

const MONITORING_TABS: Array<{ value: ResourceMonitoringTab; label: string }> = [
  { value: RESOURCE_MONITORING_TAB.OVERVIEW, label: "Overview" },
  { value: RESOURCE_MONITORING_TAB.INSTALLATIONS, label: "Installations" },
  { value: RESOURCE_MONITORING_TAB.ACTIVITY, label: "Activity" },
  { value: RESOURCE_MONITORING_TAB.USAGE, label: "Usage" },
]

export function ResourceDetailMonitoring({
  resource,
  showMemberDetail,
}: {
  resource: ManagedResource
  showMemberDetail: boolean
}) {
  const dates = useUsageRange()
  const [tab, setTab] = useState<ResourceMonitoringTab>(RESOURCE_MONITORING_TAB.OVERVIEW)
  const usage = useQuery({
    queryKey: ["resource-detail-usage", resource.id, dates.range],
    queryFn: () => api.resourceUsage({
      ...dates.range,
      resource_id: resource.id,
      limit: 50,
    }),
  })
  const inventory = useQuery({
    queryKey: ["resource-detail-inventory", resource.id],
    queryFn: () => api.resourceInventory(resource.id),
  })

  return (
    <div>
      <div className="mb-4 flex flex-col gap-3 rounded-xl border border-(--border-card) bg-(--bg-card) p-3 xl:flex-row xl:items-center xl:justify-between">
        <div>
          <div className="text-sm font-semibold">Resource monitoring</div>
          <p className="mt-0.5 text-xs text-(--color-text-muted)">Current installation inventory plus privacy-safe attributed telemetry.</p>
        </div>
        <DateRangeFilter preset={dates.preset} onPresetChange={dates.setPreset} customFrom={dates.customFrom} onCustomFromChange={dates.setCustomFrom} customTo={dates.customTo} onCustomToChange={dates.setCustomTo} />
      </div>
      <div className="mb-4 flex gap-1 overflow-x-auto border-b border-(--border-soft)">
        {MONITORING_TABS
          .filter(
            (item) =>
              showMemberDetail ||
              (item.value !== RESOURCE_MONITORING_TAB.INSTALLATIONS &&
                item.value !== RESOURCE_MONITORING_TAB.ACTIVITY),
          )
          .map((item) => (
          <button
            key={item.value}
            type="button"
            className={`border-b-2 px-3 py-2 text-xs font-medium transition-colors ${tab === item.value ? "border-(--color-accent) text-(--color-text)" : "border-transparent text-(--color-text-muted) hover:text-(--color-text)"}`}
            onClick={() => setTab(item.value)}
          >
            {item.label}
          </button>
        ))}
      </div>
      {(usage.error || inventory.error) && (
        <ErrorState message={(usage.error ?? inventory.error)?.message ?? "Resource monitoring could not be loaded."} />
      )}
      {tab === RESOURCE_MONITORING_TAB.OVERVIEW && <MonitoringOverview usage={usage.data} inventory={inventory.data} loading={usage.isLoading || inventory.isLoading} />}
      {showMemberDetail && tab === RESOURCE_MONITORING_TAB.INSTALLATIONS && <InstallationsPanel data={inventory.data} loading={inventory.isLoading} />}
      {showMemberDetail && tab === RESOURCE_MONITORING_TAB.ACTIVITY && <ActivityPanel data={usage.data} loading={usage.isLoading} />}
      {tab === RESOURCE_MONITORING_TAB.USAGE && <UsagePanel resource={resource} data={usage.data} loading={usage.isLoading} dates={dates} showMemberDetail={showMemberDetail} />}
    </div>
  )
}

function MonitoringOverview({ usage, inventory, loading }: { usage?: ResourceUsageAnalytics; inventory?: ResourceInventoryMonitoring; loading: boolean }) {
  const totals = usage?.totals
  const inventoryTotals = inventory?.summary
  const successTotal = (totals?.successes ?? 0) + (totals?.errors ?? 0)
  const successRate = successTotal ? Math.round(((totals?.successes ?? 0) / successTotal) * 100) : 0
  if (loading) return <SkeletonRows rows={5} />
  return (
    <>
      <div className="overflow-x-auto rounded-xl border border-(--border-card) bg-(--bg-card)">
        <div className="grid min-w-max auto-cols-[7.75rem] grid-flow-col divide-x divide-(--border-soft)">
          <MonitoringMetric label="Installed" value={inventoryTotals?.installed_installations ?? 0} hint={`${inventoryTotals?.installed_members ?? 0} members`} icon={Download} />
          <MonitoringMetric label="Pending" value={inventoryTotals?.pending_installations ?? 0} hint={`${inventoryTotals?.attention_installations ?? 0} need attention`} icon={Gauge} />
          <MonitoringMetric label="Requests" value={totals?.requests ?? 0} hint={`${totals?.resource_uses ?? 0} attributed uses`} icon={Activity} />
          <MonitoringMetric label="Success" value={`${successRate}%`} hint={`${totals?.errors ?? 0} errors`} icon={Gauge} />
          <MonitoringMetric label="Installed members" value={totals?.installed_members ?? 0} hint="aggregate only" icon={Users} />
          <MonitoringMetric label="Model calls" value={totals?.model_calls ?? 0} hint={`${usage?.models.length ?? 0} models`} icon={Bot} />
          <MonitoringMetric label="Tool calls" value={totals?.tool_calls ?? 0} hint={`${usage?.tools.length ?? 0} tools`} icon={Wrench} />
          <MonitoringMetric label="Est. cost" value={formatEstimatedCost(totals?.estimated_cost_usd_micros ?? 0)} hint={`${totals?.unpriced_model_calls ?? 0} unpriced`} icon={CircleDollarSign} />
        </div>
      </div>
      {!hasAnalyticsData(usage) ? (
        <TelemetryReadiness data={usage} />
      ) : (
        <div className="mt-4 grid gap-4 xl:grid-cols-2">
          <RequestOutcomeChart daily={usage?.daily ?? []} />
          <TokenCostChart daily={usage?.daily ?? []} />
        </div>
      )}
    </>
  )
}

function MonitoringMetric({ label, value, hint, icon: Icon }: { label: string; value: number | string; hint: string; icon: typeof Boxes }) {
  return (
    <div className="p-3">
      <div className="flex items-center justify-between gap-2 text-[0.68rem] text-(--color-text-muted)"><span>{label}</span><Icon className="size-3.5 text-(--color-accent)" /></div>
      <div className="mt-1 text-lg font-semibold tabular-nums">{typeof value === "number" ? value.toLocaleString() : value}</div>
      <div className="mt-0.5 truncate text-[0.65rem] text-(--color-text-subtle)">{hint}</div>
    </div>
  )
}

function InstallationsPanel({ data, loading }: { data?: ResourceInventoryMonitoring; loading: boolean }) {
  if (loading) return <SkeletonRows rows={6} />
  if (!data?.installations.length) return <EmptyState title="No reported installations" description="Install or reconcile this resource from EvoFlux to populate current inventory." />
  return (
    <Card>
      <CardHeader><div><CardTitle>Current installation inventory</CardTitle><p className="mt-0.5 text-xs text-(--color-text-muted)">One current row per EvoFlux installation. Desired and applied versions reveal update drift.</p></div><Badge tone="neutral">{data.summary.reported_installations} reported</Badge></CardHeader>
      <CardContent className="p-0">
        <TableWrap className="rounded-none border-0">
          <Table>
            <TableHead><tr><TableTh>Member / role</TableTh><TableTh>EvoFlux installation</TableTh><TableTh>Desired → applied</TableTh><TableTh>Channel</TableTh><TableTh>State</TableTh><TableTh>Observed / seen</TableTh></tr></TableHead>
            <TableBody>
              {data.installations.map((item) => (
                <TableRow key={item.installation_id}>
                  <TableTd><div className="font-medium">{item.member_name}</div><div className="text-xs text-(--color-text-subtle)">{item.email}</div><Badge tone="accent" className="mt-1">{PRIMARY_ROLE_LABELS[item.primary_role]}</Badge></TableTd>
                  <TableTd><div className="font-medium">{item.installation_name}</div><div className="text-xs capitalize text-(--color-text-subtle)">{item.platform} · EvoFlux {item.evoflux_version}</div></TableTd>
                  <TableTd className="font-mono text-xs"><div>{item.desired_version ? `v${item.desired_version}` : "—"}</div><div className="text-(--color-text-subtle)">→ {item.applied_version ? `v${item.applied_version}` : "not applied"}</div></TableTd>
                  <TableTd><Badge tone="neutral" className="capitalize">{item.release_channel ?? "—"}</Badge></TableTd>
                  <TableTd><InventoryStateBadge state={item.observed_state} />{item.error_category && <div className="mt-1 max-w-48 break-words text-xs text-(--color-danger)">{item.error_category}</div>}</TableTd>
                  <TableTd className="text-xs whitespace-nowrap text-(--color-text-muted)"><div>{new Date(item.observed_at).toLocaleString()}</div><div className="text-(--color-text-subtle)">Client seen {new Date(item.last_seen_at).toLocaleString()}</div></TableTd>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </TableWrap>
      </CardContent>
    </Card>
  )
}

function InventoryStateBadge({ state }: { state: string }) {
  const installed = (RESOURCE_INSTALLED_STATES as readonly string[]).includes(state)
  const pending = (RESOURCE_PENDING_STATES as readonly string[]).includes(state)
  return <Badge tone={installed ? "success" : pending ? "warning" : "danger"} className="capitalize">{state.replaceAll("_", " ")}</Badge>
}

function ActivityPanel({ data, loading }: { data?: ResourceUsageAnalytics; loading: boolean }) {
  if (loading) return <SkeletonRows rows={7} />
  return (
    <Card>
      <CardHeader><div><CardTitle>Attributed request activity</CardTitle><p className="mt-0.5 text-xs text-(--color-text-muted)">Click Details for the event timeline, model/tool calls, tokens, duration and cost.</p></div><Badge tone="neutral">{data?.activity_total ?? 0} rows</Badge></CardHeader>
      <CardContent className="p-0">{data?.activity.length ? <ResourceUsageActivityTable items={data.activity} /> : <EmptyState title="No attributed requests" description="Usage appears after EvoFlux reports a request that used this resource." className="border-0 py-12" />}</CardContent>
    </Card>
  )
}

function UsagePanel({
  resource,
  data,
  loading,
  dates,
  showMemberDetail,
}: {
  resource: ManagedResource
  data?: ResourceUsageAnalytics
  loading: boolean
  dates: ReturnType<typeof useUsageRange>
  showMemberDetail: boolean
}) {
  const query: AnalyticsQuery = {
    date_range: monitoringDateRange(dates.preset),
    from: dates.preset === "custom" ? dates.range.from ?? null : null,
    to: dates.preset === "custom" ? dates.range.to ?? null : null,
    resource_kind: resource.kind,
    resource_id: resource.id,
  }
  return (
    <>
      <ResourceAnalyticsStudio
        data={data}
        loading={loading}
        scopeLabel={resource.name}
        storageKey={`conductor.resource-analytics.resource.${resource.id}.v1`}
        scope={{ resourceKind: resource.kind, resourceId: resource.id }}
        query={query}
        onApplyQuery={(saved) => applyMonitoringDateRange(saved, dates)}
        allowMemberDetail={showMemberDetail}
      />
      {showMemberDetail && (
        <Breakdown title="Member adoption" description="Who used the resource, recorded role, requests, uses, tokens and estimated cost.">{data?.members.length ? <ResourceMemberBreakdownTable items={data.members} /> : <UsageEmpty />}</Breakdown>
      )}
      <Breakdown title="Calls by role" description="Requests, model calls and tool calls by the role captured at ingest time.">{data?.roles.length ? <ResourceRoleBreakdownTable items={data.roles} /> : <UsageEmpty />}</Breakdown>
      <Breakdown title="Tool calls" description="Which tools this resource drives most, including outcome, average duration and last use.">{data?.tools.length ? <ResourceToolBreakdownTable items={data.tools} /> : <UsageEmpty />}</Breakdown>
      <Breakdown title="Provider and model calls" description="Model volume, total tokens, estimated cost and pricing coverage.">{data?.models.length ? <ResourceModelBreakdownTable items={data.models} /> : <UsageEmpty />}</Breakdown>
      <div className="mt-4 grid overflow-hidden rounded-xl border border-(--border-card) bg-(--bg-card) sm:grid-cols-3 sm:divide-x sm:divide-(--border-soft)">
        <MonitoringMetric label="Total tokens" value={formatTokens(data?.totals.total_tokens ?? 0)} hint={`${formatTokens(data?.totals.average_tokens_per_request ?? 0)} average/request`} icon={Activity} />
        <MonitoringMetric label="Average duration" value={formatDuration(data?.totals.average_duration_ms ?? 0)} hint="terminal request duration" icon={Gauge} />
        <MonitoringMetric label="Estimated cost" value={formatEstimatedCost(data?.totals.estimated_cost_usd_micros ?? 0)} hint={`${data?.totals.unpriced_model_calls ?? 0} unpriced calls`} icon={CircleDollarSign} />
      </div>
    </>
  )
}

function monitoringDateRange(
  preset: ReturnType<typeof useUsageRange>["preset"],
): AnalyticsQuery["date_range"] {
  if (preset === "day") return "last_24_hours"
  if (preset === "week") return "last_7_days"
  if (preset === "custom") return "custom"
  return "last_30_days"
}

function applyMonitoringDateRange(
  query: AnalyticsQuery,
  dates: ReturnType<typeof useUsageRange>,
) {
  if (query.date_range === "last_24_hours") return dates.setPreset("day")
  if (query.date_range === "last_7_days") return dates.setPreset("week")
  if (query.date_range === "last_30_days") return dates.setPreset("month")
  dates.setPreset("custom")
  const fallback = new Date()
  const from = query.date_range === "last_90_days"
    ? new Date(fallback.getTime() - 90 * 86_400_000).toISOString().slice(0, 10)
    : query.from?.slice(0, 10)
  const to = query.date_range === "last_90_days"
    ? fallback.toISOString().slice(0, 10)
    : query.to?.slice(0, 10)
  if (from) dates.setCustomFrom(from)
  if (to) dates.setCustomTo(to)
}

function Breakdown({ title, description, children }: { title: string; description: string; children: React.ReactNode }) {
  return <Card className="mt-4"><CardHeader><div><CardTitle>{title}</CardTitle><p className="mt-0.5 text-xs text-(--color-text-muted)">{description}</p></div></CardHeader><CardContent className="p-0">{children}</CardContent></Card>
}

function UsageEmpty() {
  return <EmptyState title="No usage in this range" description="Adjust the date range or wait for attributed telemetry." className="border-0 py-10" />
}
