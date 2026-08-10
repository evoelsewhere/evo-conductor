import { useQuery } from "@tanstack/react-query"
import { Laptop, Radio } from "lucide-react"

import { api } from "@/shared/api/client"
import {
  CLIENT_PLATFORM_LABELS,
  MEMBER_PRESENCE_LABELS,
  MEMBER_PRESENCE_ONLINE_WINDOW_MS,
  MEMBER_PRESENCE_STATUS,
  MEMBER_PRESENCE_TONES,
  MEMBER_QUERY_KEYS,
  type ClientPlatform,
} from "@/shared/constants/member"
import {
  MINUTES_PER_HOUR,
  SECONDS_PER_MINUTE,
  MILLISECONDS_PER_SECOND,
  HOURS_PER_DAY,
} from "@/shared/constants/time"
import { Badge, StatusDot } from "@/shared/ui/badge"
import { EmptyState, ErrorState } from "@/shared/ui/empty-state"

export function MemberInstallationsPanel({ userId }: { userId: string }) {
  const query = useQuery({
    queryKey: MEMBER_QUERY_KEYS.installations(userId),
    queryFn: () => api.memberInstallations(userId),
  })

  return (
    <section className="space-y-2">
      <div>
        <h3 className="text-sm font-medium text-(--color-text)">EvoFlux installations</h3>
        <p className="mt-1 text-xs leading-relaxed text-(--color-text-muted)">
          Registered desktop clients. Presence is based on the most recent heartbeat.
        </p>
      </div>

      {query.isLoading && (
        <div className="space-y-2" aria-label="Loading installations">
          {[0, 1].map((item) => (
            <div
              key={item}
              className="h-16 animate-pulse rounded-lg border border-(--color-border) bg-(--bg-key)"
            />
          ))}
        </div>
      )}

      {query.error && (
        <ErrorState
          message={query.error instanceof Error ? query.error.message : "Failed to load installations"}
        />
      )}

      {query.data?.length === 0 && (
        <EmptyState
          icon={Laptop}
          title="No EvoFlux installations"
          description="This member has not registered an EvoFlux desktop client yet."
          className="py-7"
        />
      )}

      {query.data && query.data.length > 0 && (
        <ul className="divide-y divide-(--color-border-subtle) overflow-hidden rounded-lg border border-(--color-border)">
          {query.data.map((installation) => {
            const lastSeen = new Date(installation.last_seen_at)
            const online =
              Date.now() - lastSeen.getTime() <= MEMBER_PRESENCE_ONLINE_WINDOW_MS
            const presence = online
              ? MEMBER_PRESENCE_STATUS.ONLINE
              : MEMBER_PRESENCE_STATUS.OFFLINE
            return (
              <li key={installation.id} className="flex items-center gap-3 bg-(--bg-card) px-3 py-3">
                <div className="grid size-8 shrink-0 place-items-center rounded-md bg-(--bg-key) text-(--color-text-muted)">
                  <Laptop className="size-4" aria-hidden="true" />
                </div>
                <div className="min-w-0 flex-1">
                  <div className="truncate text-sm font-medium text-(--color-text)">
                    {installation.display_name}
                  </div>
                  <div className="mt-0.5 text-xs text-(--color-text-muted)">
                    {platformLabel(installation.platform)} · EvoFlux {installation.evoflux_version}
                  </div>
                  <div className="mt-0.5 text-[0.65rem] text-(--color-text-subtle)">
                    Connected {new Date(installation.connected_at).toLocaleDateString()}
                  </div>
                </div>
                <div className="shrink-0 text-right">
                  <Badge tone={MEMBER_PRESENCE_TONES[presence]}>
                    <StatusDot tone={MEMBER_PRESENCE_TONES[presence]} />
                    {MEMBER_PRESENCE_LABELS[presence]}
                  </Badge>
                  <div className="mt-1 flex items-center justify-end gap-1 text-[0.65rem] text-(--color-text-subtle)">
                    <Radio className="size-2.5" aria-hidden="true" />
                    <time dateTime={installation.last_seen_at}>{formatLastSeen(lastSeen)}</time>
                  </div>
                </div>
              </li>
            )
          })}
        </ul>
      )}
    </section>
  )
}

function platformLabel(platform: ClientPlatform) {
  return CLIENT_PLATFORM_LABELS[platform]
}

function formatLastSeen(value: Date) {
  const seconds = Math.max(
    0,
    Math.round((Date.now() - value.getTime()) / MILLISECONDS_PER_SECOND),
  )
  if (seconds < SECONDS_PER_MINUTE) return `${seconds}s ago`
  const minutes = Math.floor(seconds / SECONDS_PER_MINUTE)
  if (minutes < MINUTES_PER_HOUR) return `${minutes}m ago`
  const hours = Math.floor(minutes / MINUTES_PER_HOUR)
  if (hours < HOURS_PER_DAY) return `${hours}h ago`
  return value.toLocaleDateString()
}
