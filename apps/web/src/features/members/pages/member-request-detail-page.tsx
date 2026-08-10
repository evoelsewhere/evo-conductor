import { useQuery } from "@tanstack/react-query"
import { Link, useParams } from "@tanstack/react-router"
import { ArrowLeft, Bot, Clock3, Wrench } from "lucide-react"

import { MemberNav } from "@/features/members/components/member-nav"
import { TelemetryStatusBadge } from "@/features/members/components/telemetry-status-badge"
import { formatDuration, formatTokens } from "@/features/members/components/usage-charts"
import { api } from "@/shared/api/client"
import { PageFrame } from "@/shared/components/page-frame"
import { ProviderBrandIcon } from "@/shared/components/provider-brand-icon"
import { StatCard, StatCardGrid, StatCardGridSkeleton } from "@/shared/components/stat-card"
import {
  TELEMETRY_QUERY_KEYS,
  TELEMETRY_FALLBACK_LABELS,
  TelemetryEventType,
  TelemetryToolCategory,
} from "@/shared/constants/telemetry"
import { Card, CardContent, CardHeader, CardTitle } from "@/shared/ui/card"
import { ErrorState } from "@/shared/ui/empty-state"

export function MemberRequestDetailPage() {
  const { userId, requestId } = useParams({ strict: false }) as { userId: string; requestId: string }
  const detail = useQuery({
    queryKey: TELEMETRY_QUERY_KEYS.request(userId, requestId),
    queryFn: () => api.memberRequestDetail(userId, requestId),
  })
  const request = detail.data?.request
  const events = detail.data?.events ?? []

  return (
    <PageFrame title="Request detail" subtitle={request ? `${request.model ?? TELEMETRY_FALLBACK_LABELS.model} · ${new Date(request.started_at).toLocaleString()}` : "Model calls and tool executions for one EvoFlux request."}>
      <Link to="/app/members/$userId/activity" params={{ userId }} className="mb-3 inline-flex items-center gap-1 text-xs text-(--color-text-muted) hover:text-(--color-text)"><ArrowLeft className="size-3.5" />All activity</Link>
      <MemberNav userId={userId} />
      {detail.error && <ErrorState className="mb-4" message={detail.error.message} />}
      {detail.isLoading ? (
        <StatCardGridSkeleton count={4} />
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
          </StatCardGrid>
          <Card className="mt-4">
            <CardHeader><div><CardTitle>Execution timeline</CardTitle><p className="mt-0.5 text-xs text-(--color-text-muted)">Privacy-safe metadata only; no prompts, outputs, arguments, or file paths.</p></div></CardHeader>
            <CardContent>
              <ol className="relative ml-3 border-l border-(--color-border)">
                {events.map((event) => {
                  const isModel = event.event_type === TelemetryEventType.ModelCall
                  return (
                    <li key={event.event_id} className="relative pb-6 pl-6 last:pb-0">
                      <span className="absolute -left-3 grid size-6 place-items-center rounded-full border border-(--color-border) bg-(--bg-card)">{isModel ? <ProviderBrandIcon providerId={event.provider ?? event.model} className="size-[1.125rem] rounded-full" /> : <Wrench className="size-3" />}</span>
                      <div className="flex flex-wrap items-start justify-between gap-2">
                        <div><div className="text-sm font-medium">{isModel ? `${event.provider ?? TELEMETRY_FALLBACK_LABELS.providerName}:${event.model ?? TELEMETRY_FALLBACK_LABELS.modelIdentifier}` : event.tool_name ?? TELEMETRY_FALLBACK_LABELS.tool}</div><div className="mt-0.5 text-xs text-(--color-text-subtle)">{event.agent_name ?? TELEMETRY_FALLBACK_LABELS.agent} · {new Date(event.reported_at).toLocaleTimeString()} · {formatDuration(event.duration_ms)}</div></div>
                        <TelemetryStatusBadge status={event.status} />
                      </div>
                      {isModel ? <div className="mt-2 flex flex-wrap gap-x-4 gap-y-1 text-xs text-(--color-text-muted)"><span>{formatTokens(event.tokens_in)} input</span><span>{formatTokens(event.tokens_out)} output</span><span>{formatTokens(event.cache_read_tokens)} cache</span><span>{formatTokens(event.reasoning_tokens)} reasoning</span></div> : <div className="mt-2 text-xs text-(--color-text-muted)">Category: {event.tool_category ?? TelemetryToolCategory.Other}</div>}
                      {event.error_category && <div className="mt-1 text-xs text-(--color-error)">Error category: {event.error_category}</div>}
                    </li>
                  )
                })}
              </ol>
            </CardContent>
          </Card>
        </>
      )}
    </PageFrame>
  )
}
