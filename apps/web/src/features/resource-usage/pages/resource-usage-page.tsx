import { useQuery } from "@tanstack/react-query"
import { Link } from "@tanstack/react-router"
import {
  Activity,
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
  ResourceAnalyticsStudio,
  TelemetryReadiness,
  hasAnalyticsData,
} from "@/features/resource-usage/components/resource-analytics-studio"
import {
  ResourceBreakdownTable,
  ResourceMemberBreakdownTable,
  ResourceModelBreakdownTable,
  ResourceRoleBreakdownTable,
  ResourceToolBreakdownTable,
} from "@/features/resource-usage/components/resource-usage-breakdown-tables"
import {
  RequestOutcomeChart,
  TokenCostChart,
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
  type AnalyticsQuery,
  type PrimaryRole,
  type ResourceUsageAnalytics,
  type ResourceUsageParams,
} from "@/shared/api/client"
import { PageFrame } from "@/shared/components/page-frame"
import { PERMISSION, mayRequest } from "@/shared/lib/authorization"
import { useMinimumLoading } from "@/shared/hooks/use-minimum-loading"
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
import { LoadingState, Skeleton } from "@/shared/ui/skeleton"
import { useAuthStore } from "@/shared/stores/auth"
import { terminalRequestSuccessRate } from "@/shared/lib/telemetry-metrics"

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
    title: "Analytics Studio",
    subtitle: "Build and export custom views of adoption, reliability, tokens, cost, models and tools.",
  },
}

export function ResourceUsagePage({
  view = RESOURCE_USAGE_VIEW.OVERVIEW,
  scopeKind,
}: {
  view?: ResourceUsageView
  scopeKind?: Extract<ResourceKind, "plugin" | "skill" | "agent">
}) {
  const can = useAuthStore((state) => state.can)
  const authorization = useAuthStore((state) => state.authorization)
  const allowMemberDetail = mayRequest(can(PERMISSION.TELEMETRY_MEMBER_READ_ANY))
  const initialRange = useMemo(readRangeFromUrl, [])
  const dates = useUsageRange(initialRange.preset, initialRange.from, initialRange.to)
  const [filters, setFilters] = useState<ResourceUsageFilterState>(() =>
    sanitizeMemberFilters(readFiltersFromUrl(scopeKind), allowMemberDetail),
  )
  const [offset, setOffset] = useState(readOffsetFromUrl)
  const deferredProvider = useDeferredValue(filters.provider.trim())
  const deferredModel = useDeferredValue(filters.model.trim())
  const deferredToolName = useDeferredValue(filters.toolName.trim())
  const members = useQuery({
    queryKey: [RESOURCE_USAGE_MEMBERS_QUERY_KEY],
    queryFn: () => api.members({ limit: 100 }),
    enabled: allowMemberDetail,
  })
  const installations = useQuery({
    queryKey: [RESOURCE_USAGE_INSTALLATIONS_QUERY_KEY, filters.memberId],
    queryFn: () => api.memberInstallations(filters.memberId),
    enabled: allowMemberDetail && filters.memberId !== RESOURCE_USAGE_ALL_FILTER,
  })
  const resources = useQuery({ queryKey: [RESOURCE_USAGE_RESOURCES_QUERY_KEY], queryFn: api.resources })
  const versions = useQuery({
    queryKey: [RESOURCE_USAGE_VERSIONS_QUERY_KEY, filters.resourceId],
    queryFn: () => api.resourceVersions(filters.resourceId),
    enabled: filters.resourceId !== RESOURCE_USAGE_ALL_FILTER,
  })

  const params = useMemo<ResourceUsageParams>(() => ({
    ...dates.range,
    member_id: allowMemberDetail ? optional(filters.memberId) : undefined,
    installation_id: allowMemberDetail ? optional(filters.installationId) : undefined,
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
  }), [allowMemberDetail, dates.range, deferredModel, deferredProvider, deferredToolName, filters, offset, scopeKind, view])

  const analyticsQuery = useMemo<AnalyticsQuery>(() => ({
    date_range: analyticsDateRange(dates.preset),
    from: dates.preset === UsageRangePreset.Custom ? dates.range.from ?? null : null,
    to: dates.preset === UsageRangePreset.Custom ? dates.range.to ?? null : null,
    member_id: params.member_id ?? null,
    primary_role: params.primary_role ?? null,
    resource_kind: params.resource_kind ?? null,
    resource_id: params.resource_id ?? null,
    version_id: params.version_id ?? null,
    status: params.status ?? null,
    provider: params.provider ?? null,
    model: params.model ?? null,
    installation_id: params.installation_id ?? null,
    relation: params.relation ?? null,
    tool_name: params.tool_name ?? null,
  }), [dates.preset, dates.range.from, dates.range.to, params])

  function applyAnalyticsQuery(query: AnalyticsQuery) {
    applyAnalyticsDateRange(query, dates)
    setFilters(sanitizeMemberFilters({
      memberId: query.member_id ?? RESOURCE_USAGE_ALL_FILTER,
      installationId: query.installation_id ?? RESOURCE_USAGE_ALL_FILTER,
      primaryRole: query.primary_role ?? RESOURCE_USAGE_ALL_FILTER,
      resourceKind: scopeKind ?? query.resource_kind ?? RESOURCE_USAGE_ALL_FILTER,
      resourceId: query.resource_id ?? RESOURCE_USAGE_ALL_FILTER,
      versionId: query.version_id ?? RESOURCE_USAGE_ALL_FILTER,
      status: query.status ?? RESOURCE_USAGE_ALL_FILTER,
      relation: query.relation ?? RESOURCE_USAGE_ALL_FILTER,
      provider: query.provider ?? "",
      model: query.model ?? "",
      toolName: query.tool_name ?? "",
    }, allowMemberDetail))
    setOffset(0)
  }

  const usage = useQuery({
    queryKey: [
      RESOURCE_USAGE_QUERY_KEY,
      authorization?.current_role,
      authorization?.policy_revision,
      scopeKind ?? "all",
      view,
      params,
    ],
    queryFn: () => api.resourceUsage(params),
    placeholderData: (previousData, previousQuery) => {
      const previousKey = previousQuery?.queryKey
      return previousKey?.[1] === authorization?.current_role &&
        previousKey?.[2] === authorization?.policy_revision &&
        previousKey?.[3] === (scopeKind ?? "all") &&
        previousKey?.[4] === view
        ? previousData
        : undefined
    },
  })
  const initialLoading = useMinimumLoading(usage.isLoading && !usage.data)
  const refreshing = usage.isFetching && Boolean(usage.data)
  const fatalUsageError = Boolean(usage.error && !usage.data && !initialLoading)

  useEffect(() => {
    const search = serializeSearch(filters, dates, view === RESOURCE_USAGE_VIEW.ACTIVITY ? offset : 0)
    const suffix = search.toString()
    window.history.replaceState(null, "", `${window.location.pathname}${suffix ? `?${suffix}` : ""}`)
  }, [dates.customFrom, dates.customTo, dates.preset, filters, offset, view])

  useEffect(() => {
    if (!allowMemberDetail) {
      setFilters((current) => sanitizeMemberFilters(current, false))
    }
  }, [allowMemberDetail])

  const copy = scopedPageCopy(view, scopeKind)
  return (
    <PageFrame
      title={copy.title}
      subtitle={copy.subtitle}
      className="max-w-[100rem]"
      action={
        <DateRangeFilter
          preset={dates.preset}
          onPresetChange={(preset) => {
            dates.setPreset(preset)
            setOffset(0)
          }}
          customFrom={dates.customFrom}
          onCustomFromChange={(from) => {
            dates.setCustomFrom(from)
            setOffset(0)
          }}
          customTo={dates.customTo}
          onCustomToChange={(to) => {
            dates.setCustomTo(to)
            setOffset(0)
          }}
        />
      }
    >
      <ResourceUsageNav kind={scopeKind} />
      <ResourceUsageFilters
        value={filters}
        members={members.data?.items ?? []}
        membersLoading={members.isLoading && !members.data}
        membersError={members.error instanceof Error ? members.error.message : undefined}
        installations={installations.data ?? []}
        installationsLoading={installations.isLoading && !installations.data}
        installationsError={installations.error instanceof Error ? installations.error.message : undefined}
        resources={(resources.data ?? []).filter((item) => [RESOURCE_KIND.AGENT, RESOURCE_KIND.SKILL, RESOURCE_KIND.PLUGIN].includes(item.kind as never))}
        resourcesLoading={resources.isLoading && !resources.data}
        resourcesError={resources.error instanceof Error ? resources.error.message : undefined}
        versions={versions.data ?? []}
        versionsLoading={versions.isLoading && !versions.data}
        versionsError={versions.error instanceof Error ? versions.error.message : undefined}
        lockedKind={scopeKind}
        allowMemberDetail={allowMemberDetail}
        onChange={(next) => { setFilters(next); setOffset(0) }}
      />

      {usage.error && !initialLoading && (
        <ErrorState className="mt-4" message={usage.error.message} />
      )}
      {refreshing && (
        <p className="mt-3 text-right text-xs text-(--color-text-subtle)" role="status" aria-live="polite">
          Updating analytics…
        </p>
      )}
      {!fatalUsageError && (
      <div aria-busy={refreshing}>
          {view === RESOURCE_USAGE_VIEW.OVERVIEW && <OverviewPanel data={usage.data} loading={initialLoading} activityPath={scopeKind ? RESOURCE_KIND_USAGE_PATHS[scopeKind].activity : RESOURCE_USAGE_PATHS.activity} showMemberDetail={allowMemberDetail} />}
          {view === RESOURCE_USAGE_VIEW.ACTIVITY && (
            <ActivityPanel data={usage.data} loading={initialLoading} refreshing={refreshing} offset={offset} onOffsetChange={setOffset} />
          )}
          {view === RESOURCE_USAGE_VIEW.USAGE && (
            <UsagePanel
              data={usage.data}
              loading={initialLoading}
              scopeKind={scopeKind}
              query={analyticsQuery}
              onApplyQuery={applyAnalyticsQuery}
              showMemberDetail={allowMemberDetail}
            />
          )}
        </div>
      )}
    </PageFrame>
  )
}

function OverviewPanel({ data, loading, activityPath, showMemberDetail }: { data?: ResourceUsageAnalytics; loading: boolean; activityPath: string; showMemberDetail: boolean }) {
  const totals = data?.totals
  const successRate =
    terminalRequestSuccessRate(totals?.successes, totals?.requests) ?? 0
  const averageCost = totals?.requests
    ? Math.round(totals.estimated_cost_usd_micros / totals.requests)
    : 0

  return (
    <>
      {loading ? <StatCardGridSkeleton count={4} className="mt-4 lg:grid-cols-4" label="Loading analytics overview" /> : (
        <StatCardGrid className="mt-4 lg:grid-cols-4">
          <StatCard label="Requests" value={(totals?.requests ?? 0).toLocaleString()} hint={`${formatTokens(totals?.average_tokens_per_request ?? 0)} tokens/request`} icon={Gauge} />
          <StatCard label="Installed members" value={(totals?.installed_members ?? 0).toLocaleString()} hint="Aggregate project adoption" icon={Users} tone="accent" />
          <StatCard label="Success rate" value={`${successRate}%`} hint={`${totals?.errors ?? 0} errors · ${totals?.blocked ?? 0} blocked · ${totals?.cancelled ?? 0} cancelled`} icon={Users} tone={successRate >= 90 ? "success" : "warning"} />
          <StatCard label="Estimated cost" value={formatEstimatedCost(totals?.estimated_cost_usd_micros ?? 0)} hint={`${formatEstimatedCost(averageCost)} avg · ${totals?.unpriced_model_calls ?? 0} unpriced`} icon={CircleDollarSign} tone="warning" />
        </StatCardGrid>
      )}
      {loading ? <OperationalHealthStripSkeleton announce={false} /> : <OperationalHealthStrip data={data} />}
      {loading ? (
        <AnalyticsChartGridSkeleton announce={false} />
      ) : !hasAnalyticsData(data) ? (
        <TelemetryReadiness data={data} />
      ) : (
        <div className="mt-4 grid gap-4 xl:grid-cols-2">
          <RequestOutcomeChart daily={data?.daily ?? []} />
          <TokenCostChart daily={data?.daily ?? []} />
        </div>
      )}
      {showMemberDetail && <Card className="mt-4">
        <CardHeader>
          <div><CardTitle>Recent attributed activity</CardTitle><p className="mt-0.5 text-xs text-(--color-text-muted)">Server-received request metadata. Open any row for its privacy-safe event timeline.</p></div>
          <Link to={activityPath} search className={buttonVariants({ variant: "outline", size: "sm" })}>View all activity</Link>
        </CardHeader>
        <CardContent className="p-0">
          {loading ? <ResourceUsageTableSkeleton rows={8} columns={9} label="Loading recent attributed activity" announce={false} /> : data?.activity.length ? <ResourceUsageActivityTable items={data.activity} /> : <ResourceActivityEmpty />}
        </CardContent>
      </Card>}
    </>
  )
}

function OperationalHealthStrip({ data }: { data?: ResourceUsageAnalytics }) {
  const totals = data?.totals
  const items = [
    {
      label: "Resource uses",
      value: (totals?.resource_uses ?? 0).toLocaleString(),
      hint: `${data?.resources.length ?? 0} immutable versions`,
    },
    {
      label: "Total tokens",
      value: formatTokens(totals?.total_tokens ?? 0),
      hint: `${formatTokens(totals?.tokens_in ?? 0)} in · ${formatTokens(totals?.tokens_out ?? 0)} out`,
    },
    {
      label: "Model / tool calls",
      value: `${totals?.model_calls ?? 0} / ${totals?.tool_calls ?? 0}`,
      hint: "model calls / tool calls",
    },
    {
      label: "Average duration",
      value: formatDuration(totals?.average_duration_ms ?? 0),
      hint: "terminal request duration",
    },
  ]
  return (
    <div className="mt-3 grid overflow-hidden rounded-xl border border-(--border-card) bg-(--bg-card) sm:grid-cols-2 lg:grid-cols-4 lg:divide-x lg:divide-(--border-soft)">
      {items.map((item) => (
        <div key={item.label} className="border-b border-(--border-soft) px-4 py-3 last:border-b-0 sm:[&:nth-last-child(-n+2)]:border-b-0 lg:border-b-0">
          <div className="text-[0.68rem] font-medium text-(--color-text-muted)">{item.label}</div>
          <div className="mt-1 text-lg font-semibold tabular-nums">{item.value}</div>
          <div className="mt-0.5 text-[0.68rem] text-(--color-text-subtle)">{item.hint}</div>
        </div>
      ))}
    </div>
  )
}

function OperationalHealthStripSkeleton({ announce = true }: { announce?: boolean }) {
  return (
    <LoadingState
      label="Loading operational health"
      announce={announce}
      className="mt-3 grid overflow-hidden rounded-xl border border-(--border-card) bg-(--bg-card) sm:grid-cols-2 lg:grid-cols-4"
    >
      {Array.from({ length: 4 }, (_, index) => (
        <div key={index} className="border-b border-(--border-soft) px-4 py-3 sm:border-r lg:border-b-0">
          <Skeleton className="h-3 w-24" />
          <Skeleton className="mt-2 h-5 w-14" />
          <Skeleton className="mt-2 h-3 w-32 max-w-full" />
        </div>
      ))}
    </LoadingState>
  )
}

function AnalyticsChartGridSkeleton({ announce = true }: { announce?: boolean }) {
  return (
    <LoadingState label="Loading analytics charts" announce={announce} className="mt-4 grid gap-4 xl:grid-cols-2">
      {Array.from({ length: 2 }, (_, index) => (
        <div key={index} className="rounded-xl border border-(--border-card) bg-(--bg-card) p-4">
          <Skeleton className="h-4 w-32" />
          <Skeleton className="mt-2 h-3 w-56 max-w-full" />
          <div className="mt-5 grid h-52 grid-cols-8 items-end gap-2 border-b border-l border-(--border-soft) px-3 pb-3">
            {[38, 57, 72, 48, 83, 66, 76, 54].map((height, bar) => (
              <Skeleton key={bar} className="w-full rounded-b-none" style={{ height: `${height}%` }} />
            ))}
          </div>
        </div>
      ))}
    </LoadingState>
  )
}

function ResourceUsageTableSkeleton({
  rows,
  columns,
  label,
  announce = true,
}: {
  rows: number
  columns: number
  label: string
  announce?: boolean
}) {
  return (
    <LoadingState label={label} announce={announce} className="overflow-x-auto">
      <div className="min-w-[42rem] overflow-hidden">
        <div
          className="grid gap-4 border-b border-(--border-soft) bg-(--bg-key)/30 px-4 py-3"
          style={{ gridTemplateColumns: `repeat(${columns}, minmax(0, 1fr))` }}
        >
          {Array.from({ length: columns }, (_, index) => (
            <Skeleton key={index} className="h-3 w-16 max-w-full" />
          ))}
        </div>
        <div className="divide-y divide-(--border-soft)">
          {Array.from({ length: rows }, (_, row) => (
            <div
              key={row}
              className="grid items-center gap-4 px-4 py-3.5"
              style={{ gridTemplateColumns: `repeat(${columns}, minmax(0, 1fr))` }}
            >
              {Array.from({ length: columns }, (_, column) => (
                <Skeleton
                  key={column}
                  className={column === 0 ? "h-4 w-full" : "h-3.5 w-3/4"}
                />
              ))}
            </div>
          ))}
        </div>
      </div>
    </LoadingState>
  )
}

function ActivityPanel({
  data,
  loading,
  refreshing,
  offset,
  onOffsetChange,
}: {
  data?: ResourceUsageAnalytics
  loading: boolean
  refreshing: boolean
  offset: number
  onOffsetChange: (offset: number) => void
}) {
  const totals = data?.totals
  const lastPage = offset + RESOURCE_USAGE_PAGE_SIZE >= (data?.activity_total ?? 0)
  return (
    <>
      {loading ? <StatCardGridSkeleton count={4} className="mt-4 lg:grid-cols-4" label="Loading analytics activity" /> : (
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
          {loading ? <Skeleton className="h-5 w-14" /> : <Badge tone="neutral">{data?.activity_total ?? 0} rows</Badge>}
        </CardHeader>
        <CardContent className="p-0">
          {loading ? <ResourceUsageTableSkeleton rows={8} columns={9} label="Loading attributed request activity" announce={false} /> : data?.activity.length ? <ResourceUsageActivityTable items={data.activity} /> : <ResourceActivityEmpty />}
          {((data?.activity_total ?? 0) > RESOURCE_USAGE_PAGE_SIZE || offset > 0) && (
            <div className="flex items-center justify-between border-t border-(--border-soft) px-4 py-3 text-xs text-(--color-text-muted)">
              <span>{offset + 1}–{Math.min(offset + RESOURCE_USAGE_PAGE_SIZE, data?.activity_total ?? 0)} of {data?.activity_total ?? 0}</span>
              <div className="flex gap-2"><Button size="sm" variant="outline" disabled={refreshing || offset === 0} onClick={() => onOffsetChange(Math.max(0, offset - RESOURCE_USAGE_PAGE_SIZE))}>Previous</Button><Button size="sm" variant="outline" disabled={refreshing || lastPage} onClick={() => onOffsetChange(offset + RESOURCE_USAGE_PAGE_SIZE)}>Next</Button></div>
            </div>
          )}
        </CardContent>
      </Card>
    </>
  )
}

function UsagePanel({
  data,
  loading,
  scopeKind,
  query,
  onApplyQuery,
  showMemberDetail,
}: {
  data?: ResourceUsageAnalytics
  loading: boolean
  scopeKind?: Extract<ResourceKind, "plugin" | "skill" | "agent">
  query: AnalyticsQuery
  onApplyQuery: (query: AnalyticsQuery) => void
  showMemberDetail: boolean
}) {
  const scopeLabel = scopeKind ? `${RESOURCE_KIND_LABEL[scopeKind]}s` : "Resources"
  return (
    <>
      <span className="sr-only" role="status" aria-live="polite" aria-atomic="true">
        {loading ? "Loading analytics breakdowns…" : ""}
      </span>
      <ResourceAnalyticsStudio
        data={data}
        loading={loading}
        scopeLabel={scopeLabel}
        storageKey={`conductor.resource-analytics.${scopeKind ?? "all"}.v1`}
        scope={scopeKind ? { resourceKind: scopeKind } : undefined}
        query={query}
        onApplyQuery={onApplyQuery}
        allowMemberDetail={showMemberDetail}
        announceLoading={false}
      />
      <BreakdownCard title="Resource and version usage" description="Adoption, request outcomes, calls, token volume and cost by immutable resource version.">
        {loading ? <ResourceUsageTableSkeleton rows={5} columns={9} label="Loading resource and version usage" announce={false} /> : data?.resources.length ? <ResourceBreakdownTable items={data.resources} /> : <ResourceUsageEmpty title="No resource usage" description="Resource-version breakdown appears after attributed telemetry arrives." />}
      </BreakdownCard>
      {showMemberDetail && (
        <BreakdownCard title="Member adoption" description="Recorded role, resource usage and consumption by member; no productivity scoring.">
          {loading ? <ResourceUsageTableSkeleton rows={5} columns={7} label="Loading member adoption" announce={false} /> : data?.members.length ? <ResourceMemberBreakdownTable items={data.members} /> : <ResourceUsageEmpty title="No member adoption" description="Member breakdown appears after attributed telemetry arrives." />}
        </BreakdownCard>
      )}
      <BreakdownCard title="Provider and model usage" description="Calls, tokens, estimated cost and pricing coverage while governed resources were active.">
        {loading ? <ResourceUsageTableSkeleton rows={5} columns={7} label="Loading provider and model usage" announce={false} /> : data?.models.length ? <ResourceModelBreakdownTable items={data.models} /> : <ResourceUsageEmpty title="No model usage" description="Provider and model breakdown appears after model-call telemetry arrives." />}
      </BreakdownCard>
      <BreakdownCard title="Calls by recorded role" description="Request, model-call and tool-call volume by the role captured with each request.">
        {loading ? <ResourceUsageTableSkeleton rows={3} columns={6} label="Loading calls by recorded role" announce={false} /> : data?.roles.length ? <ResourceRoleBreakdownTable items={data.roles} /> : <ResourceUsageEmpty title="No role usage" description="Role breakdown appears after attributed telemetry arrives." />}
      </BreakdownCard>
      <BreakdownCard title="Tool call breakdown" description="Privacy-safe tool identifiers, category, outcome, latency and last-use time.">
        {loading ? <ResourceUsageTableSkeleton rows={5} columns={7} label="Loading tool call breakdown" announce={false} /> : data?.tools.length ? <ResourceToolBreakdownTable items={data.tools} /> : <ResourceUsageEmpty title="No tool calls" description="Tool breakdown appears after tool-call telemetry arrives." />}
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

function optional(value: string) {
  return value && value !== RESOURCE_USAGE_ALL_FILTER ? value : undefined
}

function sanitizeMemberFilters(
  filters: ResourceUsageFilterState,
  allowMemberDetail: boolean,
): ResourceUsageFilterState {
  if (allowMemberDetail) return filters
  return {
    ...filters,
    memberId: RESOURCE_USAGE_ALL_FILTER,
    installationId: RESOURCE_USAGE_ALL_FILTER,
  }
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
    return { title: `${name} Analytics Studio`, subtitle: `Build and export custom ${kind} adoption, reliability, cost, model and tool views.` }
  }
  return { title: `${name} monitoring`, subtitle: `Monitor governed ${kind} adoption and usage across this project.` }
}

function analyticsDateRange(preset: UsageRangePreset): AnalyticsQuery["date_range"] {
  if (preset === UsageRangePreset.Day) return "last_24_hours"
  if (preset === UsageRangePreset.Week) return "last_7_days"
  if (preset === UsageRangePreset.Custom) return "custom"
  return "last_30_days"
}

function applyAnalyticsDateRange(
  query: AnalyticsQuery,
  dates: ReturnType<typeof useUsageRange>,
) {
  if (query.date_range === "last_24_hours") {
    dates.setPreset(UsageRangePreset.Day)
    return
  }
  if (query.date_range === "last_7_days") {
    dates.setPreset(UsageRangePreset.Week)
    return
  }
  if (query.date_range === "last_30_days") {
    dates.setPreset(UsageRangePreset.Month)
    return
  }
  dates.setPreset(UsageRangePreset.Custom)
  const from = query.date_range === "last_90_days"
    ? dateInputDaysAgo(90)
    : query.from?.slice(0, 10)
  const to = query.date_range === "last_90_days"
    ? dateInputDaysAgo(0)
    : query.to?.slice(0, 10)
  if (from) dates.setCustomFrom(from)
  if (to) dates.setCustomTo(to)
}

function dateInputDaysAgo(days: number) {
  const value = new Date(Date.now() - days * 86_400_000)
  return `${value.getFullYear()}-${String(value.getMonth() + 1).padStart(2, "0")}-${String(value.getDate()).padStart(2, "0")}`
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
