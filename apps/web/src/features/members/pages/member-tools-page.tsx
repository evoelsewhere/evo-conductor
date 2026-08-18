import { useQuery } from "@tanstack/react-query"
import { Link, useParams } from "@tanstack/react-router"
import { ArrowLeft, CheckCircle2, Timer, Wrench, XCircle } from "lucide-react"

import { DateRangeFilter, useUsageRange } from "@/features/members/components/date-range-filter"
import { MemberNav } from "@/features/members/components/member-nav"
import { formatDuration, formatNumber, ToolUsageChart } from "@/features/members/components/usage-charts"
import { api } from "@/shared/api/client"
import { PageFrame } from "@/shared/components/page-frame"
import { StatCard, StatCardGrid, StatCardGridSkeleton } from "@/shared/components/stat-card"
import { MEMBER_QUERY_KEYS } from "@/shared/constants/member"
import { useMinimumLoading } from "@/shared/hooks/use-minimum-loading"
import {
  TELEMETRY_PERCENT_SCALE,
  TELEMETRY_QUERY_KEYS,
  TelemetryToolCategory,
} from "@/shared/constants/telemetry"
import { Badge } from "@/shared/ui/badge"
import { ErrorState } from "@/shared/ui/empty-state"
import { LoadingState, Skeleton } from "@/shared/ui/skeleton"
import { Table, TableBody, TableHead, TableRow, TableTd, TableTh, TableWrap } from "@/shared/ui/table"

export function MemberToolsPage() {
  const { userId } = useParams({ strict: false }) as { userId: string }
  const dates = useUsageRange()
  const member = useQuery({
    queryKey: MEMBER_QUERY_KEYS.detail(userId),
    queryFn: () => api.getMember(userId),
  })
  const tools = useQuery({
    queryKey: TELEMETRY_QUERY_KEYS.tools(userId, dates.range.from, dates.range.to),
    queryFn: () => api.memberTools(userId, dates.range),
    placeholderData: (previousData, previousQuery) =>
      previousQuery?.queryKey[1] === userId ? previousData : undefined,
  })
  const initialLoading = useMinimumLoading(tools.isLoading && !tools.data)
  const refreshing = tools.isFetching && Boolean(tools.data)
  return (
    <PageFrame title="Tools & Plugins" subtitle={member.data ? `Tool execution summary for ${member.data.display_name}` : "Tool usage, success, errors, and latency."}>
      <Link to="/app/members/$userId" params={{ userId }} className="mb-3 inline-flex items-center gap-1 text-xs text-(--color-text-muted) hover:text-(--color-text)"><ArrowLeft className="size-3.5" />Member overview</Link>
      <MemberNav userId={userId} />
      <div className="mb-4 flex justify-end"><DateRangeFilter preset={dates.preset} onPresetChange={dates.setPreset} customFrom={dates.customFrom} onCustomFromChange={dates.setCustomFrom} customTo={dates.customTo} onCustomToChange={dates.setCustomTo} /></div>
      {tools.error && !initialLoading && (
        <ErrorState className="mb-4" message={tools.error.message} />
      )}
      {refreshing && <p className="mb-2 text-right text-xs text-(--color-text-subtle)" role="status" aria-live="polite">Updating tool telemetry…</p>}
      {initialLoading ? (
        <StatCardGridSkeleton count={3} className="sm:grid-cols-3 lg:grid-cols-3" label="Loading member tool usage" />
      ) : tools.data ? (
        <StatCardGrid className="sm:grid-cols-3 lg:grid-cols-3">
          <StatCard label="Total calls" value={formatNumber(tools.data?.total_calls ?? 0)} icon={Wrench} tone="accent" />
          <StatCard label="Successful" value={formatNumber(tools.data?.successful_calls ?? 0)} hint={`${successRate(tools.data?.successful_calls ?? 0, tools.data?.total_calls ?? 0)}% success rate`} icon={CheckCircle2} tone="success" />
          <StatCard label="Failed" value={formatNumber(tools.data?.failed_calls ?? 0)} icon={XCircle} tone="warning" />
        </StatCardGrid>
      ) : null}
      {initialLoading ? (
        <MemberToolsContentSkeleton />
      ) : tools.data ? (
        <div className="mt-4 grid gap-4 lg:grid-cols-[minmax(0,0.8fr)_minmax(0,1.2fr)]">
          <ToolUsageChart tools={tools.data.tools} />
          {tools.data.tools.length > 0 && (
          <TableWrap>
            <Table>
              <TableHead><tr><TableTh>Tool</TableTh><TableTh>Category</TableTh><TableTh>Calls</TableTh><TableTh>Success</TableTh><TableTh>Avg. time</TableTh><TableTh>Last used</TableTh></tr></TableHead>
              <TableBody>
                {tools.data.tools.map((item) => (
                  <TableRow key={`${item.category}:${item.tool_name}`}>
                    <TableTd className="font-medium">{item.tool_name}</TableTd>
                    <TableTd><Badge tone={item.category === TelemetryToolCategory.Mcp ? "accent" : "neutral"}>{item.category}</Badge></TableTd>
                    <TableTd className="tabular-nums">{item.calls}</TableTd>
                    <TableTd className="tabular-nums">{successRate(item.successes, item.calls)}%</TableTd>
                    <TableTd className="tabular-nums"><span className="inline-flex items-center gap-1"><Timer className="size-3 text-(--color-text-subtle)" />{formatDuration(item.average_duration_ms)}</span></TableTd>
                    <TableTd>{new Date(item.last_used_at).toLocaleString()}</TableTd>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </TableWrap>
          )}
        </div>
      ) : null}
    </PageFrame>
  )
}

function MemberToolsContentSkeleton() {
  return (
    <LoadingState label="Loading tool usage details" announce={false} className="mt-4 grid gap-4 lg:grid-cols-[minmax(0,0.8fr)_minmax(0,1.2fr)]">
      <div className="rounded-xl border border-(--border-card) bg-(--bg-card) p-4">
        <Skeleton className="h-4 w-28" />
        <Skeleton className="mt-2 h-3 w-48 max-w-full" />
        <div className="mt-5 grid h-52 grid-cols-6 items-end gap-2 border-b border-l border-(--border-soft) px-3 pb-3">
          {[48, 72, 58, 84, 65, 76].map((height, index) => (
            <Skeleton key={index} className="w-full rounded-b-none" style={{ height: `${height}%` }} />
          ))}
        </div>
      </div>
      <TableWrap>
        <Table>
          <TableHead><tr><TableTh>Tool</TableTh><TableTh>Category</TableTh><TableTh>Calls</TableTh><TableTh>Success</TableTh><TableTh>Avg. time</TableTh><TableTh>Last used</TableTh></tr></TableHead>
          <TableBody>
            {Array.from({ length: 6 }, (_, row) => (
              <TableRow key={row}>
                {Array.from({ length: 6 }, (_, column) => (
                  <TableTd key={column}><Skeleton className={column === 0 ? "h-4 w-28" : "h-3.5 w-16"} /></TableTd>
                ))}
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </TableWrap>
    </LoadingState>
  )
}

function successRate(successes: number, total: number) {
  return total ? Math.round((successes / total) * TELEMETRY_PERCENT_SCALE) : 0
}
