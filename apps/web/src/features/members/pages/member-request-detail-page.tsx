import { useQuery } from "@tanstack/react-query"
import { Link, useParams } from "@tanstack/react-router"
import { ArrowLeft, Bot, CircleDollarSign, Clock3, Wrench } from "lucide-react"

import { MemberNav } from "@/features/members/components/member-nav"
import { formatDuration, formatTokens } from "@/features/members/components/usage-charts"
import { formatEstimatedCost } from "@/features/resource-usage/components/resource-usage-formatters"
import { RequestEventTimeline } from "@/features/resource-usage/components/request-event-timeline"
import { api } from "@/shared/api/client"
import { PageFrame } from "@/shared/components/page-frame"
import { ProviderBrandIcon } from "@/shared/components/provider-brand-icon"
import { StatCard, StatCardGrid, StatCardGridSkeleton } from "@/shared/components/stat-card"
import { useMinimumLoading } from "@/shared/hooks/use-minimum-loading"
import {
  TELEMETRY_QUERY_KEYS,
  TELEMETRY_FALLBACK_LABELS,
} from "@/shared/constants/telemetry"
import { Card, CardContent, CardHeader, CardTitle } from "@/shared/ui/card"
import { ErrorState } from "@/shared/ui/empty-state"
import { LoadingState, Skeleton } from "@/shared/ui/skeleton"

export function MemberRequestDetailPage() {
  const { userId, requestId } = useParams({ strict: false }) as { userId: string; requestId: string }
  const detail = useQuery({
    queryKey: TELEMETRY_QUERY_KEYS.request(userId, requestId),
    queryFn: () => api.memberRequestDetail(userId, requestId),
  })
  const request = detail.data?.request
  const events = detail.data?.events ?? []
  const initialLoading = useMinimumLoading(detail.isLoading && !detail.data)

  return (
    <PageFrame title="Request detail" subtitle={request ? `${request.model ?? TELEMETRY_FALLBACK_LABELS.model} · ${new Date(request.started_at).toLocaleString()}` : "Model calls and tool executions for one EvoFlux request."}>
      <Link to="/app/members/$userId/activity" params={{ userId }} className="mb-3 inline-flex items-center gap-1 text-xs text-(--color-text-muted) hover:text-(--color-text)"><ArrowLeft className="size-3.5" />All activity</Link>
      <MemberNav userId={userId} />
      {detail.error && !initialLoading && (
        <ErrorState className="mb-4" message={detail.error.message} />
      )}
      {initialLoading ? (
        <MemberRequestDetailSkeleton />
      ) : request && (
        <>
          <div className="mb-4 flex flex-wrap items-center gap-2 rounded-xl border border-(--border-card) bg-(--bg-card) px-4 py-3">
            <ProviderBrandIcon providerId={request.provider ?? request.model} size="sm" />
            <div className="mr-2 min-w-0">
              <div className="truncate text-sm font-medium">{request.model ?? TELEMETRY_FALLBACK_LABELS.model}</div>
              <div className="truncate text-xs text-(--color-text-subtle)">{request.provider ?? TELEMETRY_FALLBACK_LABELS.provider}</div>
            </div>
            <span className="text-xs text-(--color-text-subtle)">Request ID</span><code className="break-all text-xs">{request.request_id}</code>
            {request.session_id && <><span className="ml-auto text-xs text-(--color-text-subtle)">Session</span><code className="text-xs">{request.session_id}</code></>}
          </div>
          <StatCardGrid>
            <StatCard label="Total tokens" value={formatTokens(request.total_tokens)} hint={`${formatTokens(request.tokens_in)} in · ${formatTokens(request.tokens_out)} out`} icon={Bot} tone="accent" />
            <StatCard label="Model calls" value={request.model_calls} hint={`${request.provider ?? TELEMETRY_FALLBACK_LABELS.providerName}:${request.model ?? TELEMETRY_FALLBACK_LABELS.modelIdentifier}`} icon={Bot} />
            <StatCard label="Tool calls" value={request.tool_calls} hint="Names and status only" icon={Wrench} tone="success" />
            <StatCard label="Measured duration" value={formatDuration(request.duration_ms)} hint="Sum of reported operations" icon={Clock3} />
            <StatCard label="Cost" value={formatEstimatedCost(request.estimated_cost_usd_micros)} hint={`${request.unpriced_model_calls} unpriced model calls`} icon={CircleDollarSign} tone="warning" />
          </StatCardGrid>
          <Card className="mt-4">
            <CardHeader><div><CardTitle>Execution timeline</CardTitle><p className="mt-0.5 text-xs text-(--color-text-muted)">Privacy-safe metadata only; no prompts, outputs, arguments, or file paths.</p></div></CardHeader>
            <CardContent>
              <RequestEventTimeline events={events} />
            </CardContent>
          </Card>
        </>
      )}
    </PageFrame>
  )
}

function MemberRequestDetailSkeleton() {
  return (
    <LoadingState label="Loading request detail" className="grid gap-4">
      <div className="flex items-center gap-3 rounded-xl border border-(--border-card) bg-(--bg-card) px-4 py-3">
        <Skeleton className="size-8 rounded-full" />
        <div className="min-w-0 flex-1">
          <Skeleton className="h-4 w-36" />
          <Skeleton className="mt-1.5 h-3 w-24" />
        </div>
        <Skeleton className="h-3 w-48 max-w-[35%]" />
      </div>
      <StatCardGridSkeleton count={5} />
      <Card>
        <CardHeader>
          <div className="min-w-0 flex-1">
            <Skeleton className="h-4 w-32" />
            <Skeleton className="mt-2 h-3 w-72 max-w-full" />
          </div>
        </CardHeader>
        <CardContent className="grid gap-5">
          {Array.from({ length: 4 }, (_, index) => (
            <div key={index} className="grid gap-2 border-l border-(--border-soft) pl-6">
              <div className="flex items-center justify-between gap-3">
                <Skeleton className="h-4 w-40" />
                <Skeleton className="h-5 w-16" />
              </div>
              <Skeleton className="h-3 w-64 max-w-full" />
              <Skeleton className="h-3 w-3/4" />
            </div>
          ))}
        </CardContent>
      </Card>
    </LoadingState>
  )
}
