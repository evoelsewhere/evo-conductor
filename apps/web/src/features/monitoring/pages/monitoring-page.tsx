import { useQuery } from "@tanstack/react-query"
import { CircleDollarSign, Coins, Gauge, TriangleAlert } from "lucide-react"
import { useMemo, useState } from "react"

import {
  DateRangeFilter,
  useUsageRange,
} from "@/features/members/components/date-range-filter"
import { formatTokens } from "@/features/members/components/usage-formatters"
import { formatEstimatedCost } from "@/features/resource-usage/components/resource-usage-formatters"
import { CostTableSkeleton, ModelCostTable } from "@/features/monitoring/components/model-cost-table"
import { MemberCostTable } from "@/features/monitoring/components/member-cost-table"
import {
  EMPTY_MONITORING_FILTERS,
  MonitoringFilters,
  type MonitoringFilterState,
} from "@/features/monitoring/components/monitoring-filters"
import {
  api,
  type CostReportParams,
  type MemberCostReportRow,
  type ModelCostReportRow,
  type PrimaryRole,
} from "@/shared/api/client"
import { PageFrame } from "@/shared/components/page-frame"
import { useMinimumLoading } from "@/shared/hooks/use-minimum-loading"
import { StatCard, StatCardGrid, StatCardGridSkeleton } from "@/shared/components/stat-card"
import { Button } from "@/shared/ui/button"
import { Card, CardHeader, CardTitle } from "@/shared/ui/card"
import { EmptyState, ErrorState } from "@/shared/ui/empty-state"
import { MONITORING_ALL_FILTER, MONITORING_TAB, type MonitoringTab } from "@/shared/constants/monitoring"

type SortKey = "cost" | "tokens" | "rate"

const SORT_LABEL: Record<SortKey, string> = {
  cost: "Cost",
  tokens: "Tokens",
  rate: "$ / 1M",
}

function optional(value: string) {
  return value && value !== MONITORING_ALL_FILTER ? value : undefined
}

export function MonitoringPage() {
  const dates = useUsageRange()
  const [tab, setTab] = useState<MonitoringTab>(MONITORING_TAB.MODEL)
  const [sort, setSort] = useState<SortKey>("cost")
  const [filters, setFilters] = useState<MonitoringFilterState>(EMPTY_MONITORING_FILTERS)

  const tags = useQuery({ queryKey: ["tags"], queryFn: api.tags })

  const params = useMemo<CostReportParams>(
    () => ({
      ...dates.range,
      primary_role: optional(filters.primaryRole) as PrimaryRole | undefined,
      tag_id: optional(filters.tagId),
      provider: optional(filters.provider),
      model: optional(filters.model),
    }),
    [dates.range, filters],
  )

  const modelReport = useQuery({
    queryKey: ["model-cost-report", params],
    queryFn: () => api.modelCostReport(params),
    enabled: tab === MONITORING_TAB.MODEL,
    placeholderData: (previous) => previous,
  })
  const memberReport = useQuery({
    queryKey: ["member-cost-report", params],
    queryFn: () => api.memberCostReport(params),
    enabled: tab === MONITORING_TAB.MEMBER,
    placeholderData: (previous) => previous,
  })

  const active = tab === MONITORING_TAB.MODEL ? modelReport : memberReport
  const initialLoading = useMinimumLoading(active.isLoading && !active.data)
  const refreshing = active.isFetching && Boolean(active.data)
  const fatalError = Boolean(active.error && !active.data && !initialLoading)

  const modelRows = useMemo(() => sortModelRows(modelReport.data?.rows ?? [], sort), [modelReport.data?.rows, sort])
  const memberRows = useMemo(() => sortMemberRows(memberReport.data?.rows ?? [], sort), [memberReport.data?.rows, sort])

  const totals = active.data?.totals
  const avgCostPerMillion = totals?.total_tokens
    ? Math.round((totals.total_cost_usd_micros / totals.total_tokens) * 1_000_000)
    : 0

  return (
    <PageFrame
      title="Monitoring"
      subtitle="Where project spend goes, by model and by member — filter by role, tag, provider or model to narrow either view."
      className="max-w-[100rem]"
      action={
        <DateRangeFilter
          preset={dates.preset}
          onPresetChange={dates.setPreset}
          customFrom={dates.customFrom}
          onCustomFromChange={dates.setCustomFrom}
          customTo={dates.customTo}
          onCustomToChange={dates.setCustomTo}
        />
      }
    >
      <div className="mb-4 flex flex-wrap items-center justify-between gap-3">
        <div className="inline-flex rounded-md border border-(--color-border) bg-(--bg-page) p-0.5">
          <Button
            size="sm"
            variant={tab === MONITORING_TAB.MODEL ? "secondary" : "ghost"}
            aria-pressed={tab === MONITORING_TAB.MODEL}
            onClick={() => setTab(MONITORING_TAB.MODEL)}
          >
            By model
          </Button>
          <Button
            size="sm"
            variant={tab === MONITORING_TAB.MEMBER ? "secondary" : "ghost"}
            aria-pressed={tab === MONITORING_TAB.MEMBER}
            onClick={() => setTab(MONITORING_TAB.MEMBER)}
          >
            By member
          </Button>
        </div>
      </div>

      <MonitoringFilters value={filters} tags={tags.data ?? []} tagsLoading={tags.isLoading && !tags.data} onChange={setFilters} />

      {active.error && !initialLoading && (
        <ErrorState className="mt-4" message={active.error.message} />
      )}
      {refreshing && (
        <p className="mt-3 text-right text-xs text-(--color-text-subtle)" role="status" aria-live="polite">
          Updating report…
        </p>
      )}

      {!fatalError && (
        <div aria-busy={refreshing}>
          {initialLoading ? (
            <StatCardGridSkeleton count={4} className="mt-4" label="Loading cost overview" />
          ) : (
            <StatCardGrid className="mt-4">
              <StatCard
                label="Total spend"
                value={formatEstimatedCost(totals?.total_cost_usd_micros ?? 0)}
                hint={`${(totals?.calls ?? 0).toLocaleString()} model calls`}
                icon={CircleDollarSign}
              />
              <StatCard
                label="Total tokens"
                value={formatTokens(totals?.total_tokens ?? 0)}
                hint={`${formatTokens(totals?.input_tokens ?? 0)} in · ${formatTokens(totals?.output_tokens ?? 0)} out`}
                icon={Coins}
                tone="accent"
              />
              <StatCard
                label="Cache tokens"
                value={formatTokens((totals?.cache_read_tokens ?? 0) + (totals?.cache_write_tokens ?? 0))}
                hint={`${formatTokens(totals?.cache_read_tokens ?? 0)} read · ${formatTokens(totals?.cache_write_tokens ?? 0)} write`}
                icon={Gauge}
              />
              <StatCard
                label="Unpriced calls"
                value={(totals?.unpriced_calls ?? 0).toLocaleString()}
                hint={`avg $${(avgCostPerMillion / 1_000_000).toFixed(2)} / 1M tokens`}
                icon={TriangleAlert}
                tone={(totals?.unpriced_calls ?? 0) > 0 ? "warning" : "success"}
              />
            </StatCardGrid>
          )}

          <Card className="mt-4">
            <CardHeader>
              <div>
                <CardTitle>{tab === MONITORING_TAB.MODEL ? "Spend by model" : "Spend by member"}</CardTitle>
                <p className="mt-1 text-xs text-(--color-text-muted)">
                  {tab === MONITORING_TAB.MODEL
                    ? "Each model's total is the same authoritative figure every other cost view shows; the four components are a proportional split of it at the model's current rate. A model with no current rate shows its whole spend as unattributed."
                    : "One row per member. A member's calls can span many models with different rates, so this view has no per-component cost split — only the token breakdown and the one authoritative total."}
                </p>
              </div>
              <div className="flex items-center gap-2">
                <span className="text-xs font-medium text-(--color-text-subtle)">Sort</span>
                <div className="inline-flex rounded-md border border-(--color-border) bg-(--bg-page) p-0.5">
                  {(Object.keys(SORT_LABEL) as SortKey[]).map((key) => (
                    <Button
                      key={key}
                      size="sm"
                      variant={sort === key ? "secondary" : "ghost"}
                      aria-pressed={sort === key}
                      onClick={() => setSort(key)}
                    >
                      {SORT_LABEL[key]}
                    </Button>
                  ))}
                </div>
              </div>
            </CardHeader>
            {initialLoading ? (
              <CostTableSkeleton columns={tab === MONITORING_TAB.MODEL ? 12 : 9} label="Loading spend report" />
            ) : tab === MONITORING_TAB.MODEL ? (
              modelRows.length === 0 ? (
                <EmptyReport />
              ) : (
                <ModelCostTable rows={modelRows} totals={modelReport.data?.totals} />
              )
            ) : memberRows.length === 0 ? (
              <EmptyReport />
            ) : (
              <MemberCostTable rows={memberRows} totals={memberReport.data?.totals} />
            )}
          </Card>
        </div>
      )}
    </PageFrame>
  )
}

function EmptyReport() {
  return (
    <div className="p-4">
      <EmptyState
        icon={CircleDollarSign}
        title="No model calls in this range"
        description="This report follows the same window every other cost view uses. Widen the date range or clear a filter, or check back after telemetry arrives."
        className="border-0 bg-transparent py-8"
      />
    </div>
  )
}

function sortModelRows(rows: ModelCostReportRow[], sort: SortKey): ModelCostReportRow[] {
  if (sort === "tokens") return [...rows].sort((a, b) => b.total_tokens - a.total_tokens)
  if (sort === "rate") {
    return [...rows].sort(
      (a, b) => b.avg_usd_micros_per_million_tokens - a.avg_usd_micros_per_million_tokens,
    )
  }
  return rows
}

function sortMemberRows(rows: MemberCostReportRow[], sort: SortKey): MemberCostReportRow[] {
  if (sort === "tokens") return [...rows].sort((a, b) => b.total_tokens - a.total_tokens)
  if (sort === "rate") {
    return [...rows].sort(
      (a, b) => b.avg_usd_micros_per_million_tokens - a.avg_usd_micros_per_million_tokens,
    )
  }
  return rows
}
