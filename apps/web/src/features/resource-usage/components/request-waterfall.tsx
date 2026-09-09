import { formatDuration } from "@/features/members/components/usage-formatters"
import { TELEMETRY_FALLBACK_LABELS, TelemetryEventStatus, TelemetryEventType } from "@/shared/constants/telemetry"
import type { TelemetryEventDetail } from "@/shared/api/client"
import { cn } from "@/shared/lib/utils"

interface TimelineBounds {
  start: number
  end: number
}

/**
 * Events only carry `reported_at` (when they finished) and `duration_ms` —
 * there is no separate start timestamp on the wire. `start = end - duration`
 * is an approximation (it assumes the report fires the instant the
 * operation ends, with no extra queue/network lag), close enough to lay out
 * a readable timeline without EvoFlux having to report anything new.
 */
function eventSpan(event: TelemetryEventDetail): { start: number; end: number } {
  const end = Date.parse(event.reported_at)
  return { start: end - event.duration_ms, end }
}

function computeBounds(events: readonly TelemetryEventDetail[]): TimelineBounds {
  let start = Number.POSITIVE_INFINITY
  let end = Number.NEGATIVE_INFINITY
  for (const event of events) {
    const span = eventSpan(event)
    start = Math.min(start, span.start)
    end = Math.max(end, span.end)
  }
  return Number.isFinite(start) && Number.isFinite(end) ? { start, end } : { start: 0, end: 0 }
}

function barPosition(event: TelemetryEventDetail, bounds: TimelineBounds) {
  const total = bounds.end - bounds.start
  if (total <= 0) return { leftPct: 0, widthPct: 100 }
  const span = eventSpan(event)
  const leftPct = ((span.start - bounds.start) / total) * 100
  // A floor keeps a near-instant event visible as a sliver instead of vanishing.
  const widthPct = Math.max(((span.end - span.start) / total) * 100, 0.5)
  return { leftPct, widthPct }
}

function eventLabel(event: TelemetryEventDetail): string {
  switch (event.event_type) {
    case TelemetryEventType.ModelCall:
      return event.model ?? TELEMETRY_FALLBACK_LABELS.model
    case TelemetryEventType.ToolCall:
      return event.tool_name ?? TELEMETRY_FALLBACK_LABELS.tool
    default:
      return "Request"
  }
}

function eventColorClass(event: TelemetryEventDetail): string {
  if (event.status !== TelemetryEventStatus.Success) return "bg-(--color-error)"
  switch (event.event_type) {
    case TelemetryEventType.ModelCall:
      return "bg-(--color-accent)"
    case TelemetryEventType.ToolCall:
      return "bg-(--color-success)"
    default:
      return "bg-(--color-text-subtle)"
  }
}

export function RequestWaterfall({
  events,
  selectedEventId,
  onSelectEvent,
}: {
  events: readonly TelemetryEventDetail[]
  selectedEventId: string | null
  onSelectEvent: (eventId: string) => void
}) {
  if (events.length === 0) return null
  const bounds = computeBounds(events)

  return (
    <div className="overflow-x-auto rounded-lg border border-(--border-soft) bg-(--bg-key)/20">
      <div className="min-w-[420px]">
        <div className="flex border-b border-(--border-soft) px-3 py-2 text-[0.65rem] font-medium tracking-wide text-(--color-text-subtle) uppercase">
          <div className="w-40 shrink-0 sm:w-56">Event</div>
          <div className="flex-1">Timeline</div>
          <div className="w-16 shrink-0 text-right">Duration</div>
        </div>
        <div className="divide-y divide-(--border-soft)">
          {events.map((event) => {
            const { leftPct, widthPct } = barPosition(event, bounds)
            const selected = event.event_id === selectedEventId
            const label = eventLabel(event)
            return (
              <button
                key={event.event_id}
                type="button"
                onClick={() => onSelectEvent(event.event_id)}
                aria-label={`${label}, ${event.status}, duration ${formatDuration(event.duration_ms)}`}
                className={cn(
                  "flex min-h-9 w-full items-center px-3 py-1.5 text-left text-xs transition-colors hover:bg-(--bg-key)/40 focus:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-(--focus-ring)",
                  selected && "bg-(--bg-key)/60",
                )}
              >
                <div className="flex w-40 shrink-0 items-center gap-1.5 sm:w-56">
                  <span
                    className={cn("size-2 shrink-0 rounded-full", eventColorClass(event))}
                    aria-hidden="true"
                  />
                  <span className="truncate font-medium text-(--color-text)" title={label}>
                    {label}
                  </span>
                </div>
                <div className="relative h-4 flex-1">
                  <div
                    className={cn(
                      "absolute top-1/2 h-2 -translate-y-1/2 rounded-sm",
                      eventColorClass(event),
                    )}
                    style={{ left: `${leftPct}%`, width: `${widthPct}%` }}
                  />
                </div>
                <div className="w-16 shrink-0 text-right text-(--color-text-subtle) tabular-nums">
                  {formatDuration(event.duration_ms)}
                </div>
              </button>
            )
          })}
        </div>
      </div>
    </div>
  )
}
