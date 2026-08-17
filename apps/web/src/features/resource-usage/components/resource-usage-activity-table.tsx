import { Link } from "@tanstack/react-router"
import { ArrowUpRight, Eye } from "lucide-react"

import { formatDuration, formatTokens } from "@/features/members/components/usage-formatters"
import { TelemetryStatusBadge } from "@/features/members/components/telemetry-status-badge"
import {
  formatEstimatedCost,
  formatRelation,
} from "@/features/resource-usage/components/resource-usage-formatters"
import type { ResourceUsageActivityItem } from "@/shared/api/client"
import { ProviderBrandIcon } from "@/shared/components/provider-brand-icon"
import { PRIMARY_ROLE_LABELS } from "@/shared/constants/member"
import { RESOURCE_KIND_LABEL } from "@/shared/constants/resource"
import { Badge } from "@/shared/ui/badge"
import { buttonVariants } from "@/shared/ui/button"
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

export function ResourceUsageActivityTable({ items }: { items: ResourceUsageActivityItem[] }) {
  return (
    <TableWrap className="rounded-none border-0">
      <Table>
        <TableHead>
          <tr>
            <TableTh>Time / member</TableTh>
            <TableTh>Resource version</TableTh>
            <TableTh>Model</TableTh>
            <TableTh>Calls</TableTh>
            <TableTh>Tokens</TableTh>
            <TableTh>Est. cost</TableTh>
            <TableTh>Duration</TableTh>
            <TableTh>Outcome</TableTh>
            <TableTh><span className="sr-only">Actions</span></TableTh>
          </tr>
        </TableHead>
        <TableBody>
          {items.map((item) => (
            <TableRow key={`${item.request_id}:${item.resource_id}:${item.version_id}:${item.relation}`}>
              <TableTd>
                <Link to="/app/resources/usage/activity/$userId/$requestId" params={{ userId: item.user_id, requestId: item.request_id }} search className="font-medium hover:text-(--color-accent)">
                  {new Date(item.occurred_at).toLocaleString()}
                </Link>
                <div className="mt-0.5 max-w-48 truncate font-mono text-[0.65rem] text-(--color-text-subtle)">{item.request_id}</div>
                <div className="mt-1 flex items-center gap-1.5 text-xs text-(--color-text-subtle)">
                  <Link to="/app/members/$userId" params={{ userId: item.user_id }} className="hover:text-(--color-accent)">{item.display_name}</Link>
                  <Badge tone="neutral">{PRIMARY_ROLE_LABELS[item.primary_role]}</Badge>
                </div>
              </TableTd>
              <TableTd>
                <div className="font-medium">{item.resource_name}</div>
                <div className="mt-0.5 flex items-center gap-1.5 text-xs text-(--color-text-subtle)">
                  <Badge tone="accent">{RESOURCE_KIND_LABEL[item.kind]}</Badge>
                  <span>v{item.version}</span>
                  <span>· {formatRelation(item.relation)}</span>
                </div>
              </TableTd>
              <TableTd>
                <div className="flex items-center gap-2">
                  <ProviderBrandIcon providerId={item.provider ?? item.model} />
                  <div><div className="font-medium">{item.model ?? "No model"}</div><div className="text-xs text-(--color-text-subtle)">{item.provider ?? "—"}</div></div>
                </div>
              </TableTd>
              <TableTd className="tabular-nums"><div>{item.model_calls} model</div><div className="text-xs text-(--color-text-subtle)">{item.tool_calls} tool</div></TableTd>
              <TableTd className="font-medium tabular-nums">{formatTokens(item.total_tokens)}</TableTd>
              <TableTd className="tabular-nums">
                {formatEstimatedCost(item.estimated_cost_usd_micros)}
                {item.unpriced_model_calls > 0 && <div className="text-xs text-(--color-warning)">{item.unpriced_model_calls} unpriced</div>}
              </TableTd>
              <TableTd className="tabular-nums">{formatDuration(item.duration_ms)}</TableTd>
              <TableTd><TelemetryStatusBadge status={item.status} /></TableTd>
              <TableTd>
                <div className="flex items-center justify-end gap-1">
                  <Link
                    to="/app/resources/$kind/$resourceId"
                    params={{ kind: item.kind, resourceId: item.resource_id }}
                    className={cn(buttonVariants({ variant: "ghost", size: "icon" }), "size-8")}
                    aria-label={`Open ${item.resource_name}`}
                    title="Open resource"
                  >
                    <ArrowUpRight className="size-3.5" />
                  </Link>
                  <Link
                    to="/app/resources/usage/activity/$userId/$requestId"
                    params={{ userId: item.user_id, requestId: item.request_id }}
                    search
                    className={cn(buttonVariants({ variant: "outline", size: "sm" }), "h-8")}
                  >
                    <Eye className="size-3.5" />Details
                  </Link>
                </div>
              </TableTd>
            </TableRow>
          ))}
        </TableBody>
      </Table>
    </TableWrap>
  )
}
