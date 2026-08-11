import { useQuery } from "@tanstack/react-query"
import { Link } from "@tanstack/react-router"
import {
  Activity,
  Bot,
  Boxes,
  CircleDollarSign,
  Clock3,
  Gauge,
  Users,
  Wrench,
} from "lucide-react"
import { useDeferredValue, useEffect, useMemo, useState } from "react"

import {
  DateRangeFilter,
  useUsageRange,
} from "@/features/members/components/date-range-filter"
import {
  formatDuration,
  formatTokens,
} from "@/features/members/components/usage-formatters"
import { ResourceUsageActivityTable } from "@/features/resource-usage/components/resource-usage-activity-table"
import {
  ResourceBreakdownTable,
  ResourceMemberBreakdownTable,
  ResourceModelBreakdownTable,
  ResourceRoleBreakdownTable,
  ResourceToolBreakdownTable,
} from "@/features/resource-usage/components/resource-usage-breakdown-tables"
import {
  ModelCallsChart,
  MemberUsageChart,
  RequestOutcomeChart,
  ResourceShareChart,
  RoleCallsChart,
  TokenCostChart,
  ToolCallsChart,
} from "@/features/resource-usage/components/resource-usage-charts"
import {
  EMPTY_RESOURCE_USAGE_FILTERS,
  ResourceUsageFilters,
  type ResourceUsageFilterState,
} from "@/features/resource-usage/components/resource-usage-filters"
import { formatEstimatedCost } from "@/features/resource-usage/components/resource-usage-formatters"
import { ResourceUsageNav } from "@/features/resource-usage/components/resource-usage-nav"
import {
  api,
  type PrimaryRole,
  type ResourceUsageAnalytics,
  type ResourceUsageParams,
} from "@/shared/api/client"
import { PageFrame } from "@/shared/components/page-frame"
import { StatCard, StatCardGrid, StatCardGridSkeleton } from "@/shared/components/stat-card"
import { RESOURCE_KIND, RESOURCE_KIND_LABEL, type ResourceKind } from "@/shared/constants/resource"
import { RESOURCE_KIND_USAGE_PATHS } from "@/shared/constants/resource-monitoring"
import {
  RESOURCE_USAGE_ALL_FILTER,
  RESOURCE_USAGE_INSTALLATIONS_QUERY_KEY,
  RESOURCE_USAGE_MEMBERS_QUERY_KEY,
  RESOURCE_USAGE_OVERVIEW_ACTIVITY_LIMIT,
  RESOURCE_USAGE_PAGE_SIZE,
  RESOURCE_USAGE_PATHS,
  RESOURCE_USAGE_PERCENT_SCALE,
  RESOURCE_USAGE_QUERY_KEY,
  RESOURCE_USAGE_RESOURCES_QUERY_KEY,
  RESOURCE_USAGE_VERSIONS_QUERY_KEY,
  RESOURCE_USAGE_VIEW,
  type ResourceUsageView,
} from "@/shared/constants/resource-usage"
import {
  DEFAULT_USAGE_RANGE_PRESET,
  UsageRangePreset,
  type TelemetryEventStatus,
} from "@/shared/constants/telemetry"
import { Badge } from "@/shared/ui/badge"
import { Button, buttonVariants } from "@/shared/ui/button"
import { Card, CardContent, CardHeader, CardTitle } from "@/shared/ui/card"
import { EmptyState, ErrorState } from "@/shared/ui/empty-state"

const PAGE_COPY: Record<ResourceUsageView, { title: string; subtitle: string }> = {
  [RESOURCE_USAGE_VIEW.OVERVIEW]: {
    title: "Resource monitoring",
    subtitle: "Monitor governed Agent, Skill and Plugin activity across this project.",
  },
  [RESOURCE_USAGE_VIEW.ACTIVITY]: {
    title: "Resource activity",
    subtitle: "Audit who used each resource version, when it ran, and the request outcome.",
  },
  [RESOURCE_USAGE_VIEW.USAGE]: {
    title: "Resource usage",
    subtitle: "Analyze adoption, tokens, model and tool calls, estimated cost, and failures.",
  },
}

export function ResourceUsagePage({
  view = RESOURCE_USAGE_VIEW.OVERVIEW,
  scopeKind,
}: {
  view?: ResourceUsageView
  scopeKind?: Extract<ResourceKind, "plugin" | "skill" | "agent">
}) {
  const initialRange = useMemo(readRangeFromUrl, [])
  const dates = useUsageRange(initialRange.preset, initialRange.from, initialRange.to)
  const [filters, setFilters] = useState<ResourceUsageFilterState>(() => readFiltersFromUrl(scopeKind))
  const [offset, setOffset] = useState(readOffsetFromUrl)
  const deferredProvider = useDeferredValue(filters.provider.trim())
  const deferredModel = useDeferredValue(filters.model.trim())
  const deferredToolName = useDeferredValue(filters.toolName.trim())
  const members = useQuery({
    queryKey: [RESOURCE_USAGE_MEMBERS_QUERY_KEY],
    queryFn: () => api.members({ limit: 100 }),
  })
  const installations = useQuery({
    queryKey: [RESOURCE_USAGE_INSTALLATIONS_QUERY_KEY, filters.memberId],
    queryFn: () => api.memberInstallations(filters.memberId),
    enabled: filters.memberId !== RESOURCE_USAGE_ALL_FILTER,
  })
  const resources = useQuery({ queryKey: [RESOURCE_USAGE_RESOURCES_QUERY_KEY], queryFn: api.resources })
  const versions = useQuery({
    queryKey: [RESOURCE_USAGE_VERSIONS_QUERY_KEY, filters.resourceId],
    queryFn: () => api.resourceVersions(filters.resourceId),
    enabled: filters.resourceId !== RESOURCE_USAGE_ALL_FILTER,
  })

  const params = useMemo<ResourceUsageParams>(() => ({
    ...dates.range,
    member_id: optional(filters.memberId),
    installation_id: optional(filters.installationId),
    primary_role: optional(filters.primaryRole) as PrimaryRole | undefined,
    resource_kind: scopeKind ?? optional(filters.resourceKind) as ResourceKind | undefined,
    resource_id: optional(filters.resourceId),
    version_id: optional(filters.versionId),
    status: optional(filters.status) as TelemetryEventStatus | undefined,
    relation: optional(filters.relation) as ResourceUsageParams["relation"],
    provider: deferredProvider || undefined,
    model: deferredModel || undefined,
    tool_name: deferredToolName || undefined,
    limit: view === RESOURCE_USAGE_VIEW.OVERVIEW
      ? RESOURCE_USAGE_OVERVIEW_ACTIVITY_LIMIT
      : RESOURCE_USAGE_PAGE_SIZE,
    offset: view === RESOURCE_USAGE_VIEW.ACTIVITY ? offset : 0,
  }), [dates.range, deferredModel, deferredProvider, deferredToolName, filters, offset, view])

  const usage = useQuery({
    queryKey: [RESOURCE_USAGE_QUERY_KEY, params],
    queryFn: () => api.resourceUsage(params),
  })

  useEffect(() => {
    const search = serializeSearch(filters, dates, view === RESOURCE_USAGE_VIEW.ACTIVITY ? offset : 0)
    const suffix = search.toString()
    window.history.replaceState(null, "", `${window.location.pathname}${suffix ? `?${suffix}` : ""}`)
  }, [dates.customFrom, dates.customTo, dates.preset, filters, offset, view])

  const copy = scopedPageCopy(view, scopeKind)
  return (
    <PageFrame
      title={copy.title}
      subtitle={copy.subtitle}
      className="max-w-[100rem]"
      action={<DateRangeFilter preset={dates.preset} onPresetChange={dates.setPreset} customFrom={dates.customFrom} onCustomFromChange={dates.setCustomFrom} customTo={dates.customTo} onCustomToChange={dates.setCustomTo} />}
    >
      <ResourceUsageNav kind={scopeKind} />
      <ResourceUsageFilters
        value={filters}
        members={members.data?.items ?? []}
        installations={installations.data ?? []}
        resources={(resources.data ?? []).filter((item) => [RESOURCE_KIND.AGENT, RESOURCE_KIND.SKILL, RESOURCE_KIND.PLUGIN].includes(item.kind as never))}
        versions={versions.data ?? []}
        lockedKind={scopeKind}
        onChange={(next) => { setFilters(next); setOffset(0) }}
      />

      {usage.error && <ErrorState className="mt-4" message={usage.error.message} />}
      {view === RESOURCE_USAGE_VIEW.OVERVIEW && <OverviewPanel data={usage.data} loading={usage.isLoading} activityPath={scopeKind ? RESOURCE_KIND_USAGE_PATHS[scopeKind].activity : RESOURCE_USAGE_PATHS.activity} />}
      {view === RESOURCE_USAGE_VIEW.ACTIVITY && (
        <ActivityPanel data={usage.data} loading={usage.isLoading} offset={offset} onOffsetChange={setOffset} />
      )}
      {view === RESOURCE_USAGE_VIEW.USAGE && <UsagePanel data={usage.data} loading={usage.isLoading} />}
    </PageFrame>
  )
}

function OverviewPanel({ data, loading, activityPath }: { data?: ResourceUsageAnalytics; loading: boolean; activityPath: string }) {
  const totals = data?.totals
  const successRate = calculateSuccessRate(totals?.successes ?? 0, totals?.errors ?? 0)
  const averageCost = totals?.requests
    ? Math.round(totals.estimated_cost_usd_micros / totals.requests)
    : 0

  return (
    <>
      {loading ? <StatCardGridSkeleton count={6} className="mt-4 lg:grid-cols-3 xl:grid-cols-6" /> : (
        <StatCardGrid className="mt-4 lg:grid-cols-3 xl:grid-cols-6">
          <StatCard label="Requests" value={(totals?.requests ?? 0).toLocaleString()} hint={`${formatTokens(totals?.average_tokens_per_request ?? 0)} tokens/request`} icon={Gauge} />
          <StatCard label="Resource uses" value={(totals?.resource_uses ?? 0).toLocaleString()} hint={`${data?.resources.length ?? 0} attributed versions`} icon={Boxes} tone="success" />
          <StatCard label="Total tokens" value={formatTokens(totals?.total_tokens ?? 0)} hint={`${formatTokens(totals?.tokens_in ?? 0)} in · ${formatTokens(totals?.tokens_out ?? 0)} out`} icon={Activity} tone="accent" />
          <StatCard label="Model / tool calls" value={`${totals?.model_calls ?? 0} / ${totals?.tool_calls ?? 0}`} hint="model calls / tool calls" icon={Bot} />
          <StatCard label="Success rate" value={`${successRate}%`} hint={`${totals?.errors ?? 0} errors · ${totals?.blocked ?? 0} blocked · ${totals?.cancelled ?? 0} cancelled`} icon={Users} tone={successRate >= 90 ? "success" : "warning"} />
          <StatCard label="Estimated cost" value={formatEstimatedCost(totals?.estimated_cost_usd_micros ?? 0)} hint={`${formatEstimatedCost(averageCost)} avg · ${totals?.unpriced_model_calls ?? 0} unpriced`} icon={CircleDollarSign} tone="warning" />
        </StatCardGrid>
      )}
      <div className="mt-4 grid gap-4 xl:grid-cols-2">
        <RequestOutcomeChart daily={data?.daily ?? []} />
        <TokenCostChart daily={data?.daily ?? []} />
      </div>
      <Card className="mt-4">
        <CardHeader>
          <div><CardTitle>Recent attributed activity</CardTitle><p className="mt-0.5 text-xs text-(--color-text-muted)">Server-received request metadata. Open any row for its privacy-safe event timeline.</p></div>
          <Link to={activityPath} search className={buttonVariants({ variant: "outline", size: "sm" })}>View all activity</Link>
        </CardHeader>
        <CardContent className="p-0">
          {loading ? <div className="grid h-40 place-items-center text-sm text-(--color-text-muted)">Loading activity…</div> : data?.activity.length ? <ResourceUsageActivityTable items={data.activity} /> : <ResourceActivityEmpty />}
        </CardContent>
      </Card>
    </>
  )
}

function ActivityPanel({
  data,
  loading,
  offset,
  onOffsetChange,
}: {
  data?: ResourceUsageAnalytics
  loading: boolean
  offset: number
  onOffsetChange: (offset: number) => void
}) {
  const totals = data?.totals
  const lastPage = offset + RESOURCE_USAGE_PAGE_SIZE >= (data?.activity_total ?? 0)
  return (
    <>
      {loading ? <StatCardGridSkeleton count={4} className="mt-4 lg:grid-cols-4" /> : (
        <StatCardGrid className="mt-4 lg:grid-cols-4">
          <StatCard label="Attributed rows" value={(data?.activity_total ?? 0).toLocaleString()} hint="request · resource · version · relation" icon={Activity} />
          <StatCard label="Errors / blocked" value={`${totals?.errors ?? 0} / ${totals?.blocked ?? 0}`} hint={`${totals?.cancelled ?? 0} cancelled`} icon={Gauge} tone={(totals?.errors ?? 0) > 0 ? "warning" : "success"} />
          <StatCard label="Average duration" value={formatDuration(totals?.average_duration_ms ?? 0)} hint="terminal request duration" icon={Clock3} />
          <StatCard label="Model / tool calls" value={`${totals?.model_calls ?? 0} / ${totals?.tool_calls ?? 0}`} hint="all attributed calls" icon={Wrench} tone="accent" />
        </StatCardGrid>
      )}
      <Card className="mt-4">
        <CardHeader>
          <div><CardTitle>Attributed request activity</CardTitle><p className="mt-0.5 text-xs text-(--color-text-muted)">One row per request, resource version and attribution relation. Role is captured at ingest time.</p></div>
          <Badge tone="neutral">{data?.activity_total ?? 0} rows</Badge>
        </CardHeader>
        <CardContent className="p-0">
          {loading ? <div className="grid h-48 place-items-center text-sm text-(--color-text-muted)">Loading activity…</div> : data?.activity.length ? <ResourceUsageActivityTable items={data.activity} /> : <ResourceActivityEmpty />}
          {(data?.activity_total ?? 0) > RESOURCE_USAGE_PAGE_SIZE && (
            <div className="flex items-center justify-between border-t border-(--border-soft) px-4 py-3 text-xs text-(--color-text-muted)">
              <span>{offset + 1}–{Math.min(offset + RESOURCE_USAGE_PAGE_SIZE, data?.activity_total ?? 0)} of {data?.activity_total ?? 0}</span>
              <div className="flex gap-2"><Button size="sm" variant="outline" disabled={offset === 0} onClick={() => onOffsetChange(Math.max(0, offset - RESOURCE_USAGE_PAGE_SIZE))}>Previous</Button><Button size="sm" variant="outline" disabled={lastPage} onClick={() => onOffsetChange(offset + RESOURCE_USAGE_PAGE_SIZE)}>Next</Button></div>
            </div>
          )}
        </CardContent>
      </Card>
    </>
  )
}

function UsagePanel({ data, loading }: { data?: ResourceUsageAnalytics; loading: boolean }) {
  const totals = data?.totals
  return (
    <>
      {loading ? <StatCardGridSkeleton count={6} className="mt-4 lg:grid-cols-3 xl:grid-cols-6" /> : (
        <StatCardGrid className="mt-4 lg:grid-cols-3 xl:grid-cols-6">
          <StatCard label="Input tokens" value={formatTokens(totals?.tokens_in ?? 0)} hint="model input" icon={Activity} />
          <StatCard label="Output tokens" value={formatTokens(totals?.tokens_out ?? 0)} hint="model output" icon={Activity} tone="accent" />
          <StatCard label="Cache read" value={formatTokens(totals?.cache_read_tokens ?? 0)} hint="reused context" icon={Gauge} />
          <StatCard label="Reasoning" value={formatTokens(totals?.reasoning_tokens ?? 0)} hint="reported reasoning tokens" icon={Bot} />
          <StatCard label="Tool-use tokens" value={formatTokens(totals?.tool_use_tokens ?? 0)} hint={`${totals?.tool_calls ?? 0} tool calls`} icon={Wrench} tone="success" />
          <StatCard label="Estimated cost" value={formatEstimatedCost(totals?.estimated_cost_usd_micros ?? 0)} hint={`${totals?.unpriced_model_calls ?? 0} unpriced model calls`} icon={CircleDollarSign} tone="warning" />
        </StatCardGrid>
      )}
      <div className="mt-4 grid gap-4 xl:grid-cols-2">
        <ResourceShareChart resources={data?.resources ?? []} />
        <MemberUsageChart members={data?.members ?? []} />
        <RoleCallsChart roles={data?.roles ?? []} />
        <ToolCallsChart tools={data?.tools ?? []} />
        <ModelCallsChart models={data?.models ?? []} />
      </div>
      <BreakdownCard title="Resource and version usage" description="Adoption, request outcomes, calls, token volume and cost by immutable resource version.">
        {data?.resources.length ? <ResourceBreakdownTable items={data.resources} /> : <ResourceUsageEmpty title="No resource usage" description="Resource-version breakdown appears after attributed telemetry arrives." />}
      </BreakdownCard>
      <BreakdownCard title="Member adoption" description="Recorded role, resource usage and consumption by member; no productivity scoring.">
        {data?.members.length ? <ResourceMemberBreakdownTable items={data.members} /> : <ResourceUsageEmpty title="No member adoption" description="Member breakdown appears after attributed telemetry arrives." />}
      </BreakdownCard>
      <BreakdownCard title="Provider and model usage" description="Calls, tokens, estimated cost and pricing coverage while governed resources were active.">
        {data?.models.length ? <ResourceModelBreakdownTable items={data.models} /> : <ResourceUsageEmpty title="No model usage" description="Provider and model breakdown appears after model-call telemetry arrives." />}
      </BreakdownCard>
      <BreakdownCard title="Calls by recorded role" description="Request, model-call and tool-call volume by the role captured with each request.">
        {data?.roles.length ? <ResourceRoleBreakdownTable items={data.roles} /> : <ResourceUsageEmpty title="No role usage" description="Role breakdown appears after attributed telemetry arrives." />}
      </BreakdownCard>
      <BreakdownCard title="Tool call breakdown" description="Privacy-safe tool identifiers, category, outcome, latency and last-use time.">
        {data?.tools.length ? <ResourceToolBreakdownTable items={data.tools} /> : <ResourceUsageEmpty title="No tool calls" description="Tool breakdown appears after tool-call telemetry arrives." />}
      </BreakdownCard>
    </>
  )
}

function BreakdownCard({ title, description, children }: { title: string; description: string; children: React.ReactNode }) {
  return <Card className="mt-4"><CardHeader><div><CardTitle>{title}</CardTitle><p className="mt-0.5 text-xs text-(--color-text-muted)">{description}</p></div></CardHeader><CardContent className="p-0">{children}</CardContent></Card>
}

function ResourceActivityEmpty() {
  return <EmptyState title="No attributed requests" description="Run a Conductor-managed Agent, activate a managed Skill, or call a managed Plugin tool from EvoFlux." className="border-0 py-12" />
}

function ResourceUsageEmpty({ title, description }: { title: string; description: string }) {
  return <EmptyState title={title} description={description} className="border-0 py-12" />
}

function calculateSuccessRate(successes: number, errors: number) {
  const completed = successes + errors
  return completed ? Math.round((successes / completed) * RESOURCE_USAGE_PERCENT_SCALE) : 0
}

function optional(value: string) {
  return value && value !== RESOURCE_USAGE_ALL_FILTER ? value : undefined
}

function readFiltersFromUrl(scopeKind?: Extract<ResourceKind, "plugin" | "skill" | "agent">): ResourceUsageFilterState {
  const search = new URLSearchParams(window.location.search)
  return {
    ...EMPTY_RESOURCE_USAGE_FILTERS,
    memberId: search.get("member_id") ?? RESOURCE_USAGE_ALL_FILTER,
    installationId: search.get("installation_id") ?? RESOURCE_USAGE_ALL_FILTER,
    primaryRole: search.get("primary_role") ?? RESOURCE_USAGE_ALL_FILTER,
    resourceKind: scopeKind ?? search.get("resource_kind") ?? RESOURCE_USAGE_ALL_FILTER,
    resourceId: search.get("resource_id") ?? RESOURCE_USAGE_ALL_FILTER,
    versionId: search.get("version_id") ?? RESOURCE_USAGE_ALL_FILTER,
    status: search.get("status") ?? RESOURCE_USAGE_ALL_FILTER,
    relation: search.get("relation") ?? RESOURCE_USAGE_ALL_FILTER,
    provider: search.get("provider") ?? "",
    model: search.get("model") ?? "",
    toolName: search.get("tool_name") ?? "",
  }
}

function scopedPageCopy(
  view: ResourceUsageView,
  kind?: Extract<ResourceKind, "plugin" | "skill" | "agent">,
) {
  if (!kind) return PAGE_COPY[view]
  const name = `${RESOURCE_KIND_LABEL[kind]}s`
  if (view === RESOURCE_USAGE_VIEW.ACTIVITY) {
    return { title: `${name} activity`, subtitle: `Audit who used each ${kind} version, when it ran, and the request outcome.` }
  }
  if (view === RESOURCE_USAGE_VIEW.USAGE) {
    return { title: `${name} usage`, subtitle: `Analyze ${kind} adoption, calls, tokens, cost, roles, and failure patterns.` }
  }
  return { title: `${name} monitoring`, subtitle: `Monitor governed ${kind} adoption and usage across this project.` }
}

function readRangeFromUrl() {
  const search = new URLSearchParams(window.location.search)
  const value = search.get("range")
  const preset = Object.values(UsageRangePreset).includes(value as UsageRangePreset)
    ? value as UsageRangePreset
    : DEFAULT_USAGE_RANGE_PRESET
  return {
    preset,
    from: search.get("from") ?? undefined,
    to: search.get("to") ?? undefined,
  }
}

function readOffsetFromUrl() {
  const parsed = Number(new URLSearchParams(window.location.search).get("offset") ?? 0)
  return Number.isSafeInteger(parsed) && parsed > 0 ? parsed : 0
}

function serializeSearch(
  filters: ResourceUsageFilterState,
  dates: ReturnType<typeof useUsageRange>,
  offset: number,
) {
  const search = new URLSearchParams()
  if (dates.preset !== DEFAULT_USAGE_RANGE_PRESET) search.set("range", dates.preset)
  if (dates.preset === UsageRangePreset.Custom) {
    search.set("from", dates.customFrom)
    search.set("to", dates.customTo)
  }
  if (offset > 0) search.set("offset", String(offset))
  if (filters.memberId !== RESOURCE_USAGE_ALL_FILTER) search.set("member_id", filters.memberId)
  if (filters.installationId !== RESOURCE_USAGE_ALL_FILTER) search.set("installation_id", filters.installationId)
  if (filters.primaryRole !== RESOURCE_USAGE_ALL_FILTER) search.set("primary_role", filters.primaryRole)
  if (filters.resourceKind !== RESOURCE_USAGE_ALL_FILTER) search.set("resource_kind", filters.resourceKind)
  if (filters.resourceId !== RESOURCE_USAGE_ALL_FILTER) search.set("resource_id", filters.resourceId)
  if (filters.versionId !== RESOURCE_USAGE_ALL_FILTER) search.set("version_id", filters.versionId)
  if (filters.status !== RESOURCE_USAGE_ALL_FILTER) search.set("status", filters.status)
  if (filters.relation !== RESOURCE_USAGE_ALL_FILTER) search.set("relation", filters.relation)
  if (filters.provider) search.set("provider", filters.provider)
  if (filters.model) search.set("model", filters.model)
  if (filters.toolName) search.set("tool_name", filters.toolName)
  return search
}
