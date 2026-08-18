import { Link } from "@tanstack/react-router"

import { formatDuration, formatTokens } from "@/features/members/components/usage-formatters"
import { TelemetryStatusBadge } from "@/features/members/components/telemetry-status-badge"
import type { MemberActivityItem } from "@/shared/api/client"
import { ProviderBrandIcon } from "@/shared/components/provider-brand-icon"
import { TELEMETRY_FALLBACK_LABELS } from "@/shared/constants/telemetry"
import { cn } from "@/shared/lib/utils"
import {
  Table,
  TableBody,
  TableHead,
  TableRow,
  TableTd,
  TableTh,
  TableWrap,
} from "@/shared/ui/table"
import { LoadingState, Skeleton } from "@/shared/ui/skeleton"

export function MemberActivityTable({
  userId,
  items,
  density = "detailed",
  className,
}: {
  userId: string
  items: MemberActivityItem[]
  density?: "compact" | "detailed"
  className?: string
}) {
  const compact = density === "compact"

  return (
    <TableWrap className={cn(compact && "rounded-none border-0", className)}>
      <Table>
        <TableHead>
          <tr>
            <TableTh>{compact ? "Time" : "Started"}</TableTh>
            <TableTh>Model</TableTh>
            {!compact && <TableTh>Input</TableTh>}
            {!compact && <TableTh>Output</TableTh>}
            <TableTh>{compact ? "Tokens" : "Total"}</TableTh>
            <TableTh>Tools</TableTh>
            {!compact && <TableTh>Duration</TableTh>}
            <TableTh>Status</TableTh>
          </tr>
        </TableHead>
        <TableBody>
          {items.map((item) => (
            <TableRow key={item.request_id}>
              <TableTd>
                <Link
                  to="/app/members/$userId/activity/$requestId"
                  params={{ userId, requestId: item.request_id }}
                  className="font-medium hover:text-(--color-accent)"
                >
                  {new Date(item.started_at).toLocaleString()}
                </Link>
                <div
                  className={cn(
                    "text-xs text-(--color-text-subtle)",
                    !compact && "max-w-40 truncate font-mono text-[0.65rem]",
                  )}
                >
                  {compact ? formatDuration(item.duration_ms) : item.request_id}
                </div>
              </TableTd>
              <TableTd>
                <div className="flex items-center gap-2">
                  <ProviderBrandIcon providerId={item.provider ?? item.model} />
                  <div className="min-w-0">
                    <div className="truncate font-medium">
                      {item.model ?? TELEMETRY_FALLBACK_LABELS.modelName}
                    </div>
                    <div className="truncate text-xs text-(--color-text-subtle)">
                      {item.provider ?? TELEMETRY_FALLBACK_LABELS.provider}
                      {!compact && ` · ${item.model_calls} call${item.model_calls === 1 ? "" : "s"}`}
                    </div>
                  </div>
                </div>
              </TableTd>
              {!compact && <TableTd className="tabular-nums">{formatTokens(item.tokens_in)}</TableTd>}
              {!compact && <TableTd className="tabular-nums">{formatTokens(item.tokens_out)}</TableTd>}
              <TableTd className={cn("tabular-nums", !compact && "font-medium")}>
                {formatTokens(item.total_tokens)}
              </TableTd>
              <TableTd className="tabular-nums">{item.tool_calls}</TableTd>
              {!compact && <TableTd className="tabular-nums">{formatDuration(item.duration_ms)}</TableTd>}
              <TableTd><TelemetryStatusBadge status={item.status} /></TableTd>
            </TableRow>
          ))}
        </TableBody>
      </Table>
    </TableWrap>
  )
}

export function MemberActivityTableSkeleton({
  rows = 6,
  density = "detailed",
  className,
  announce = true,
}: {
  rows?: number
  density?: "compact" | "detailed"
  className?: string
  announce?: boolean
}) {
  const compact = density === "compact"
  return (
    <LoadingState label="Loading member activity" announce={announce}>
      <TableWrap className={cn(compact && "rounded-none border-0", className)}>
        <Table>
          <TableHead>
            <tr>
              <TableTh>{compact ? "Time" : "Started"}</TableTh>
              <TableTh>Model</TableTh>
              {!compact && <TableTh>Input</TableTh>}
              {!compact && <TableTh>Output</TableTh>}
              <TableTh>{compact ? "Tokens" : "Total"}</TableTh>
              <TableTh>Tools</TableTh>
              {!compact && <TableTh>Duration</TableTh>}
              <TableTh>Status</TableTh>
            </tr>
          </TableHead>
          <TableBody>
            {Array.from({ length: rows }, (_, row) => (
              <TableRow key={row}>
                <TableTd><Skeleton className="h-4 w-28" /><Skeleton className="mt-1.5 h-3 w-20" /></TableTd>
                <TableTd><Skeleton className="h-4 w-24" /><Skeleton className="mt-1.5 h-3 w-16" /></TableTd>
                {!compact && <TableTd><Skeleton className="h-3.5 w-14" /></TableTd>}
                {!compact && <TableTd><Skeleton className="h-3.5 w-14" /></TableTd>}
                <TableTd><Skeleton className="h-3.5 w-14" /></TableTd>
                <TableTd><Skeleton className="h-3.5 w-8" /></TableTd>
                {!compact && <TableTd><Skeleton className="h-3.5 w-14" /></TableTd>}
                <TableTd><Skeleton className="h-5 w-16" /></TableTd>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </TableWrap>
    </LoadingState>
  )
}
