import { useQuery } from "@tanstack/react-query"
import { Link, useParams } from "@tanstack/react-router"
import { ArrowLeft, Search } from "lucide-react"
import { useMemo, useState } from "react"

import {
  DateRangeFilter,
  useUsageRange,
} from "@/features/members/components/date-range-filter"
import { MemberActivityTable } from "@/features/members/components/member-activity-table"
import { MemberNav } from "@/features/members/components/member-nav"
import { api } from "@/shared/api/client"
import { PageFrame } from "@/shared/components/page-frame"
import { MEMBER_QUERY_KEYS } from "@/shared/constants/member"
import {
  TELEMETRY_ACTIVITY_PAGE_SIZE,
  TELEMETRY_QUERY_KEYS,
  TELEMETRY_STATUS_FILTER_ALL,
  TELEMETRY_STATUS_OPTIONS,
  type TelemetryStatusFilter,
} from "@/shared/constants/telemetry"
import { EmptyState, ErrorState } from "@/shared/ui/empty-state"
import { Input } from "@/shared/ui/input"
import { Select } from "@/shared/ui/select"
import { SkeletonRows } from "@/shared/ui/skeleton"
import { TableWrap } from "@/shared/ui/table"

export function MemberActivityPage() {
  const { userId } = useParams({ strict: false }) as { userId: string }
  const dates = useUsageRange()
  const [status, setStatus] = useState<TelemetryStatusFilter>(TELEMETRY_STATUS_FILTER_ALL)
  const [modelQuery, setModelQuery] = useState("")
  const member = useQuery({
    queryKey: MEMBER_QUERY_KEYS.detail(userId),
    queryFn: () => api.getMember(userId),
  })
  const activity = useQuery({
    queryKey: TELEMETRY_QUERY_KEYS.activity(
      userId,
      dates.range.from,
      dates.range.to,
      TELEMETRY_ACTIVITY_PAGE_SIZE,
    ),
    queryFn: () =>
      api.memberActivity(userId, {
        ...dates.range,
        limit: TELEMETRY_ACTIVITY_PAGE_SIZE,
      }),
  })
  const items = useMemo(() => {
    const query = modelQuery.trim().toLowerCase()
    return (activity.data?.items ?? []).filter((item) => {
      if (status !== TELEMETRY_STATUS_FILTER_ALL && item.status !== status) return false
      if (!query) return true
      return `${item.provider ?? ""}:${item.model ?? ""}`.toLowerCase().includes(query)
    })
  }, [activity.data?.items, modelQuery, status])

  return (
    <PageFrame title="Member activity" subtitle={member.data ? `Request history for ${member.data.display_name}` : "Model and tool activity grouped by EvoFlux request."}>
      <Link to="/app/members/$userId" params={{ userId }} className="mb-3 inline-flex items-center gap-1 text-xs text-(--color-text-muted) hover:text-(--color-text)"><ArrowLeft className="size-3.5" />Member overview</Link>
      <MemberNav userId={userId} />

      <div className="mb-4 rounded-xl border border-(--border-card) bg-(--bg-card) p-3">
        <div className="flex flex-wrap items-center justify-between gap-3">
          <DateRangeFilter preset={dates.preset} onPresetChange={dates.setPreset} customFrom={dates.customFrom} onCustomFromChange={dates.setCustomFrom} customTo={dates.customTo} onCustomToChange={dates.setCustomTo} />
          <div className="flex flex-wrap gap-2">
            <div className="relative w-52"><Search className="pointer-events-none absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2 text-(--color-text-subtle)" /><Input className="pl-8" placeholder="Filter model" value={modelQuery} onChange={(event) => setModelQuery(event.target.value)} /></div>
            <Select
              value={status}
              onValueChange={(value) => setStatus(value as TelemetryStatusFilter)}
              options={[...TELEMETRY_STATUS_OPTIONS]}
            />
          </div>
        </div>
      </div>

      {activity.error && <ErrorState className="mb-4" message={activity.error.message} />}
      {activity.isLoading ? (
        <TableWrap><SkeletonRows rows={8} /></TableWrap>
      ) : items.length === 0 ? (
        <EmptyState title="No requests match" description="Adjust the date, model, or status filter." />
      ) : (
        <MemberActivityTable userId={userId} items={items} />
      )}
      {activity.data && <p className="mt-3 text-xs text-(--color-text-subtle)">Showing {items.length} of {activity.data.total} requests in this range.</p>}
    </PageFrame>
  )
}
