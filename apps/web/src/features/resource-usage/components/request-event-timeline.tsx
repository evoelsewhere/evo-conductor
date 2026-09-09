import { Activity, Wrench } from "lucide-react"
import { useRef, useState } from "react"

import { TelemetryStatusBadge } from "@/features/members/components/telemetry-status-badge"
import { formatDuration, formatTokens } from "@/features/members/components/usage-formatters"
import { RequestWaterfall } from "@/features/resource-usage/components/request-waterfall"
import {
  formatEstimatedCost,
  formatRelation,
} from "@/features/resource-usage/components/resource-usage-formatters"
import type { TelemetryEventDetail } from "@/shared/api/client"
import { ProviderBrandIcon } from "@/shared/components/provider-brand-icon"
import { cn } from "@/shared/lib/utils"
import { RESOURCE_KIND_LABEL } from "@/shared/constants/resource"
import { RESOURCE_USAGE_COST_SOURCE_LABELS } from "@/shared/constants/resource-usage"
import {
  TELEMETRY_FALLBACK_LABELS,
  TelemetryEventType,
  TelemetryToolCategory,
} from "@/shared/constants/telemetry"
import { Badge } from "@/shared/ui/badge"

/**
 * One request's full event history: a waterfall showing when each event ran
 * relative to the others, and a sequential list with the per-event detail
 * (token categories, cost, sanitized errors, resource attribution).
 * Selecting a bar in the waterfall highlights and scrolls to its row below —
 * shared by the member and resource activity detail pages, which otherwise
 * duplicated this rendering.
 */
export function RequestEventTimeline({ events }: { events: TelemetryEventDetail[] }) {
  const [selectedEventId, setSelectedEventId] = useState<string | null>(null)
  const itemRefs = useRef(new Map<string, HTMLLIElement>())

  function selectEvent(eventId: string) {
    setSelectedEventId(eventId)
    itemRefs.current.get(eventId)?.scrollIntoView({ behavior: "smooth", block: "nearest" })
  }

  return (
    <div className="flex flex-col gap-4">
      <RequestWaterfall
        events={events}
        selectedEventId={selectedEventId}
        onSelectEvent={selectEvent}
      />
      <ol className="relative ml-3 border-l border-(--color-border)">
        {events.map((event) => {
          const isModel = event.event_type === TelemetryEventType.ModelCall
          const isRequest = event.event_type === TelemetryEventType.Request
          const title = isModel
            ? `${event.provider ?? TELEMETRY_FALLBACK_LABELS.providerName}:${event.model ?? TELEMETRY_FALLBACK_LABELS.modelIdentifier}`
            : isRequest
              ? "Request completed"
              : (event.tool_name ?? TELEMETRY_FALLBACK_LABELS.tool)
          return (
            <li
              key={event.event_id}
              ref={(node) => {
                if (node) itemRefs.current.set(event.event_id, node)
                else itemRefs.current.delete(event.event_id)
              }}
              className={cn(
                "relative rounded-md pb-7 pl-6 transition-colors last:pb-0",
                event.event_id === selectedEventId && "bg-(--bg-key)/40 ring-1 ring-(--focus-ring)",
              )}
            >
              <span className="absolute -left-3 grid size-6 place-items-center rounded-full border border-(--color-border) bg-(--bg-card)">
                {isModel ? (
                  <ProviderBrandIcon
                    providerId={event.provider ?? event.model}
                    className="size-[1.125rem] rounded-full"
                  />
                ) : isRequest ? (
                  <Activity className="size-3" />
                ) : (
                  <Wrench className="size-3" />
                )}
              </span>
              <div className="flex flex-wrap items-start justify-between gap-2">
                <div>
                  <div className="text-sm font-medium">{title}</div>
                  <div className="mt-0.5 text-xs text-(--color-text-subtle)">
                    Sequence {event.sequence} · {event.agent_name ?? TELEMETRY_FALLBACK_LABELS.agent} ·{" "}
                    {new Date(event.reported_at).toLocaleString()} · {formatDuration(event.duration_ms)}
                  </div>
                </div>
                <TelemetryStatusBadge status={event.status} />
              </div>
              {isModel && (
                <div className="mt-2 grid gap-1.5 rounded-lg border border-(--border-soft) bg-(--bg-key)/25 p-2.5 text-xs text-(--color-text-muted) sm:grid-cols-2 lg:grid-cols-3">
                  <span>{formatTokens(event.tokens_in)} input</span>
                  <span>{formatTokens(event.tokens_out)} output</span>
                  <span>{formatTokens(event.cache_read_tokens)} cache read</span>
                  <span>{formatTokens(event.cache_write_tokens)} cache write</span>
                  <span>{formatTokens(event.reasoning_tokens)} reasoning</span>
                  <span>{formatTokens(event.tool_use_tokens)} tool use</span>
                  <span>
                    {event.estimated_cost_usd_micros == null
                      ? RESOURCE_USAGE_COST_SOURCE_LABELS.unpriced
                      : `${formatEstimatedCost(event.estimated_cost_usd_micros)} · ${RESOURCE_USAGE_COST_SOURCE_LABELS[event.cost_source ?? "evoflux_catalog"]}`}
                  </span>
                  {event.response_model && event.response_model !== event.model && (
                    <span className="sm:col-span-2 lg:col-span-3">
                      Response model: <code>{event.response_model}</code>
                    </span>
                  )}
                </div>
              )}
              {!isModel && !isRequest && (
                <div className="mt-2 text-xs text-(--color-text-muted)">
                  Tool category: {event.tool_category ?? TelemetryToolCategory.Other}
                </div>
              )}
              {event.resources.length > 0 && (
                <div className="mt-2 flex flex-wrap gap-1.5">
                  {event.resources.map((resource) => (
                    <Badge
                      key={`${resource.resource_id}:${resource.version_id}:${resource.relation}`}
                      tone="accent"
                    >
                      {RESOURCE_KIND_LABEL[resource.kind]} · {resource.name} v{resource.version} ·{" "}
                      {formatRelation(resource.relation)}
                    </Badge>
                  ))}
                </div>
              )}
              {event.error_category && (
                <div className="mt-2 rounded-md border border-(--color-error)/20 bg-(--color-error-subtle) px-2.5 py-1.5 text-xs text-(--color-error)">
                  Sanitized error category: {event.error_category}
                </div>
              )}
            </li>
          )
        })}
      </ol>
    </div>
  )
}
