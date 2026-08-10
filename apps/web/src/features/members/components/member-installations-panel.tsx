import { useQuery } from "@tanstack/react-query"
import { Laptop, Radio } from "lucide-react"

import { api } from "@/shared/api/client"
import { Badge, StatusDot } from "@/shared/ui/badge"
import { EmptyState, ErrorState } from "@/shared/ui/empty-state"

const ONLINE_WINDOW_MS = 150_000

export function MemberInstallationsPanel({ userId }: { userId: string }) {
  const query = useQuery({
    queryKey: ["member-installations", userId],
    queryFn: () => api.memberInstallations(userId),
  })

  return (
    <section className="space-y-2 border-t border-(--color-border-subtle) pt-4">
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
            const online = Date.now() - lastSeen.getTime() <= ONLINE_WINDOW_MS
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
                  <Badge tone={online ? "success" : "neutral"}>
                    <StatusDot tone={online ? "success" : "neutral"} />
                    {online ? "Online" : "Offline"}
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

function platformLabel(platform: "macos" | "linux" | "windows") {
  if (platform === "macos") return "macOS"
  if (platform === "windows") return "Windows"
  return "Linux"
}

function formatLastSeen(value: Date) {
  const seconds = Math.max(0, Math.round((Date.now() - value.getTime()) / 1_000))
  if (seconds < 60) return `${seconds}s ago`
  const minutes = Math.floor(seconds / 60)
  if (minutes < 60) return `${minutes}m ago`
  const hours = Math.floor(minutes / 60)
  if (hours < 24) return `${hours}h ago`
  return value.toLocaleDateString()
}
