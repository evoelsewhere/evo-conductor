import { useQuery } from "@tanstack/react-query"
import { Link, useParams } from "@tanstack/react-router"
import {
  Activity,
  ArrowLeft,
  Bot,
  CircleDollarSign,
  Clock3,
  ExternalLink,
  Wrench,
} from "lucide-react"
import { useMemo } from "react"

import { TelemetryStatusBadge } from "@/features/members/components/telemetry-status-badge"
import { formatDuration, formatTokens } from "@/features/members/components/usage-formatters"
import { formatEstimatedCost, formatRelation } from "@/features/resource-usage/components/resource-usage-formatters"
import { ResourceUsageNav } from "@/features/resource-usage/components/resource-usage-nav"
import { api } from "@/shared/api/client"
import { PageFrame } from "@/shared/components/page-frame"
import { ProviderBrandIcon } from "@/shared/components/provider-brand-icon"
import { StatCard, StatCardGrid, StatCardGridSkeleton } from "@/shared/components/stat-card"
import { MEMBER_QUERY_KEYS } from "@/shared/constants/member"
import { RESOURCE_KIND_LABEL } from "@/shared/constants/resource"
import { RESOURCE_USAGE_COST_SOURCE_LABELS, RESOURCE_USAGE_PATHS } from "@/shared/constants/resource-usage"
import {
  TELEMETRY_FALLBACK_LABELS,
  TELEMETRY_QUERY_KEYS,
  TelemetryEventType,
  TelemetryToolCategory,
} from "@/shared/constants/telemetry"
import { Badge } from "@/shared/ui/badge"
import { Card, CardContent, CardHeader, CardTitle } from "@/shared/ui/card"
import { ErrorState } from "@/shared/ui/empty-state"

export function ResourceRequestDetailPage() {
  const { userId, requestId } = useParams({ strict: false }) as { userId: string; requestId: string }
  const detail = useQuery({
    queryKey: TELEMETRY_QUERY_KEYS.request(userId, requestId),
    queryFn: () => api.memberRequestDetail(userId, requestId),
  })
  const member = useQuery({
    queryKey: MEMBER_QUERY_KEYS.detail(userId),
    queryFn: () => api.getMember(userId),
  })
  const request = detail.data?.request
  const events = detail.data?.events ?? []
  const resources = useMemo(() => {
    const unique = new Map<string, (typeof events)[number]["resources"][number]>()
    events.flatMap((event) => event.resources).forEach((resource) => {
      unique.set(`${resource.resource_id}:${resource.version_id}:${resource.relation}`, resource)
    })
    return [...unique.values()]
  }, [events])
  const tokenTotals = useMemo(() => events.reduce((totals, event) => ({
    input: totals.input + event.tokens_in,
    output: totals.output + event.tokens_out,
    cache: totals.cache + event.cache_read_tokens,
    reasoning: totals.reasoning + event.reasoning_tokens,
    toolUse: totals.toolUse + event.tool_use_tokens,
  }), { input: 0, output: 0, cache: 0, reasoning: 0, toolUse: 0 }), [events])
  const allTokens = tokenTotals.input + tokenTotals.output + tokenTotals.cache + tokenTotals.reasoning + tokenTotals.toolUse

  return (
    <PageFrame
      title="Resource activity detail"
      subtitle={request ? `${request.model ?? TELEMETRY_FALLBACK_LABELS.model} · ${new Date(request.started_at).toLocaleString()}` : "One privacy-safe EvoFlux request timeline."}
      className="max-w-[100rem]"
    >
      <Link to={RESOURCE_USAGE_PATHS.activity} search className="mb-3 inline-flex items-center gap-1 text-xs text-(--color-text-muted) hover:text-(--color-text)"><ArrowLeft className="size-3.5" />All resource activity</Link>
      <ResourceUsageNav />
      {detail.error && <ErrorState className="mb-4" message={detail.error.message} />}
      {detail.isLoading ? <StatCardGridSkeleton count={5} /> : request && (
        <>
          <div className="mb-4 flex flex-wrap items-center gap-x-3 gap-y-2 rounded-xl border border-(--border-card) bg-(--bg-card) px-4 py-3">
            <ProviderBrandIcon providerId={request.provider ?? request.model} size="sm" />
            <div className="mr-2 min-w-0"><div className="truncate text-sm font-medium">{request.model ?? TELEMETRY_FALLBACK_LABELS.model}</div><div className="truncate text-xs text-(--color-text-subtle)">{request.provider ?? TELEMETRY_FALLBACK_LABELS.provider}</div></div>
            <span className="text-xs text-(--color-text-subtle)">Request</span><code className="max-w-full break-all text-xs">{request.request_id}</code>
            {request.session_id && <><span className="text-xs text-(--color-text-subtle) xl:ml-auto">Session</span><code className="max-w-full break-all text-xs">{request.session_id}</code></>}
          </div>

          <StatCardGrid className="lg:grid-cols-5">
            <StatCard label="Total tokens" value={formatTokens(allTokens)} hint={`${formatTokens(tokenTotals.input)} in · ${formatTokens(tokenTotals.output)} out`} icon={Bot} tone="accent" />
            <StatCard label="Model calls" value={request.model_calls} hint={`${tokenTotals.cache.toLocaleString()} cache · ${tokenTotals.reasoning.toLocaleString()} reasoning`} icon={Bot} />
            <StatCard label="Tool calls" value={request.tool_calls} hint={`${formatTokens(tokenTotals.toolUse)} tool-use tokens`} icon={Wrench} tone="success" />
            <StatCard label="Duration" value={formatDuration(request.duration_ms)} hint={`${events.length} reported events`} icon={Clock3} />
            <StatCard label="Estimated cost" value={formatEstimatedCost(request.estimated_cost_usd_micros)} hint={`${request.unpriced_model_calls} unpriced model calls`} icon={CircleDollarSign} tone="warning" />
          </StatCardGrid>

          <div className="mt-4 grid gap-4 xl:grid-cols-[minmax(0,1.35fr)_minmax(20rem,0.65fr)]">
            <Card>
              <CardHeader><div><CardTitle>Execution timeline</CardTitle><p className="mt-0.5 text-xs text-(--color-text-muted)">Sequence, model routing, token categories, cost, tool status and sanitized errors only.</p></div><TelemetryStatusBadge status={request.status} /></CardHeader>
              <CardContent>
                <ol className="relative ml-3 border-l border-(--color-border)">
                  {events.map((event) => {
                    const isModel = event.event_type === TelemetryEventType.ModelCall
                    const isRequest = event.event_type === TelemetryEventType.Request
                    const title = isModel
                      ? `${event.provider ?? TELEMETRY_FALLBACK_LABELS.providerName}:${event.model ?? TELEMETRY_FALLBACK_LABELS.modelIdentifier}`
                      : isRequest ? "Request completed" : event.tool_name ?? TELEMETRY_FALLBACK_LABELS.tool
                    return (
                      <li key={event.event_id} className="relative pb-7 pl-6 last:pb-0">
                        <span className="absolute -left-3 grid size-6 place-items-center rounded-full border border-(--color-border) bg-(--bg-card)">{isModel ? <ProviderBrandIcon providerId={event.provider ?? event.model} className="size-[1.125rem] rounded-full" /> : isRequest ? <Activity className="size-3" /> : <Wrench className="size-3" />}</span>
                        <div className="flex flex-wrap items-start justify-between gap-2">
                          <div><div className="text-sm font-medium">{title}</div><div className="mt-0.5 text-xs text-(--color-text-subtle)">Sequence {event.sequence} · {event.agent_name ?? TELEMETRY_FALLBACK_LABELS.agent} · {new Date(event.reported_at).toLocaleString()} · {formatDuration(event.duration_ms)}</div></div>
                          <TelemetryStatusBadge status={event.status} />
                        </div>
                        {isModel && (
                          <div className="mt-2 grid gap-1.5 rounded-lg border border-(--border-soft) bg-(--bg-key)/25 p-2.5 text-xs text-(--color-text-muted) sm:grid-cols-2 lg:grid-cols-3">
                            <span>{formatTokens(event.tokens_in)} input</span><span>{formatTokens(event.tokens_out)} output</span><span>{formatTokens(event.cache_read_tokens)} cache read</span><span>{formatTokens(event.reasoning_tokens)} reasoning</span><span>{formatTokens(event.tool_use_tokens)} tool use</span><span>{event.estimated_cost_usd_micros == null ? RESOURCE_USAGE_COST_SOURCE_LABELS.unpriced : `${formatEstimatedCost(event.estimated_cost_usd_micros)} · ${RESOURCE_USAGE_COST_SOURCE_LABELS[event.cost_source ?? "evoflux_catalog"]}`}</span>
                            {event.response_model && event.response_model !== event.model && <span className="sm:col-span-2 lg:col-span-3">Response model: <code>{event.response_model}</code></span>}
                          </div>
                        )}
                        {!isModel && !isRequest && <div className="mt-2 text-xs text-(--color-text-muted)">Tool category: {event.tool_category ?? TelemetryToolCategory.Other}</div>}
                        {event.resources.length > 0 && <div className="mt-2 flex flex-wrap gap-1.5">{event.resources.map((resource) => <Badge key={`${resource.resource_id}:${resource.version_id}:${resource.relation}`} tone="accent">{RESOURCE_KIND_LABEL[resource.kind]} · {resource.name} v{resource.version} · {formatRelation(resource.relation)}</Badge>)}</div>}
                        {event.error_category && <div className="mt-2 rounded-md border border-(--color-error)/20 bg-(--color-error-subtle) px-2.5 py-1.5 text-xs text-(--color-error)">Sanitized error category: {event.error_category}</div>}
                      </li>
                    )
                  })}
                </ol>
              </CardContent>
            </Card>

            <div className="space-y-4">
              <Card>
                <CardHeader><CardTitle>Member attribution</CardTitle></CardHeader>
                <CardContent>
                  <div className="text-sm font-medium">{member.data?.display_name ?? "Loading member…"}</div>
                  <div className="mt-0.5 text-xs text-(--color-text-subtle)">{member.data?.email}</div>
                  {member.data && <div className="mt-3 flex items-center justify-between"><Badge tone="accent" className="capitalize">Current role: {member.data.primary_role}</Badge><Link to="/app/members/$userId" params={{ userId }} className="inline-flex items-center gap-1 text-xs text-(--color-accent)">Member profile<ExternalLink className="size-3" /></Link></div>}
                </CardContent>
              </Card>
              <Card>
                <CardHeader><div><CardTitle>Governed resources</CardTitle><p className="mt-0.5 text-xs text-(--color-text-muted)">{resources.length} immutable resource attribution{resources.length === 1 ? "" : "s"} in this request.</p></div></CardHeader>
                <CardContent className="space-y-2">
                  {resources.map((resource) => (
                    <Link key={`${resource.resource_id}:${resource.version_id}:${resource.relation}`} to="/app/resources/$kind/$resourceId/edit" params={{ kind: resource.kind, resourceId: resource.resource_id }} className="flex items-center justify-between gap-3 rounded-lg border border-(--border-soft) bg-(--bg-key)/25 p-3 hover:border-(--color-accent)/50">
                      <div className="min-w-0"><div className="truncate text-sm font-medium">{resource.name}</div><div className="mt-0.5 text-xs text-(--color-text-subtle)">{RESOURCE_KIND_LABEL[resource.kind]} · v{resource.version} · {formatRelation(resource.relation)}</div></div><ExternalLink className="size-3.5 shrink-0 text-(--color-text-subtle)" />
                    </Link>
                  ))}
                </CardContent>
              </Card>
              <Card>
                <CardHeader><CardTitle>Privacy boundary</CardTitle></CardHeader>
                <CardContent className="text-xs leading-5 text-(--color-text-muted)">Conductor stores operational metadata only. Prompts, responses, reasoning text, tool arguments/results, secret values and file paths are not displayed or retained here.</CardContent>
              </Card>
            </div>
          </div>
        </>
      )}
    </PageFrame>
  )
}
