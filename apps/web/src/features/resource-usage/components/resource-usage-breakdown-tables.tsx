import { Link } from "@tanstack/react-router"
import { ArrowUpRight, Eye } from "lucide-react"

import { formatDuration, formatTokens } from "@/features/members/components/usage-formatters"
import {
  formatEstimatedCost,
  formatRelation,
} from "@/features/resource-usage/components/resource-usage-formatters"
import type {
  ResourceUsageBreakdown,
  ResourceUsageMember,
  ResourceUsageModel,
  ResourceUsageRole,
  ResourceUsageTool,
} from "@/shared/api/client"
import { ProviderBrandIcon } from "@/shared/components/provider-brand-icon"
import { PRIMARY_ROLE_LABELS } from "@/shared/constants/member"
import { RESOURCE_KIND_LABEL } from "@/shared/constants/resource"
import { RESOURCE_USAGE_PATHS } from "@/shared/constants/resource-usage"
import { cn } from "@/shared/lib/utils"
import { Badge } from "@/shared/ui/badge"
import { buttonVariants } from "@/shared/ui/button"
import {
  Table,
  TableBody,
  TableHead,
  TableRow,
  TableTd,
  TableTh,
  TableWrap,
} from "@/shared/ui/table"

export function ResourceBreakdownTable({ items }: { items: ResourceUsageBreakdown[] }) {
  return (
    <TableWrap className="rounded-none border-0">
      <Table>
        <TableHead>
          <tr>
            <TableTh>Resource / attribution</TableTh>
            <TableTh>Adoption</TableTh>
            <TableTh>Requests / uses</TableTh>
            <TableTh>Model / tool calls</TableTh>
            <TableTh>Outcome</TableTh>
            <TableTh>Tokens</TableTh>
            <TableTh>Est. cost</TableTh>
            <TableTh>Last used</TableTh>
            <TableTh><span className="sr-only">Actions</span></TableTh>
          </tr>
        </TableHead>
        <TableBody>
          {items.map((item) => {
            const completed = item.successes + item.errors
            const successRate = completed ? Math.round((item.successes / completed) * 100) : 0
            return (
              <TableRow key={`${item.resource_id}:${item.version_id}:${item.relation}`}>
                <TableTd>
                  <Link
                    to="/app/resources/$kind/$resourceId"
                    params={{ kind: item.kind, resourceId: item.resource_id }}
                    className="font-medium hover:text-(--color-accent)"
                  >
                    {item.name}
                  </Link>
                  <div className="mt-1 flex flex-wrap items-center gap-1.5 text-xs text-(--color-text-subtle)">
                    <Badge tone="accent">{RESOURCE_KIND_LABEL[item.kind]}</Badge>
                    <span>v{item.version}</span>
                    <span>· {formatRelation(item.relation)}</span>
                  </div>
                </TableTd>
                <TableTd className="tabular-nums"><div className="font-medium">{item.members} members</div><div className="text-xs text-(--color-text-subtle)">distinct users</div></TableTd>
                <TableTd className="tabular-nums"><div>{item.requests} requests</div><div className="text-xs text-(--color-text-subtle)">{item.uses} attributed uses</div></TableTd>
                <TableTd className="tabular-nums"><div>{item.model_calls} model</div><div className="text-xs text-(--color-text-subtle)">{item.tool_calls} tool</div></TableTd>
                <TableTd className="tabular-nums"><div className="font-medium">{successRate}% success</div><div className="text-xs text-(--color-text-subtle)">{item.successes} success · {item.errors} error</div></TableTd>
                <TableTd className="font-medium tabular-nums">{formatTokens(item.total_tokens)}</TableTd>
                <TableTd className="tabular-nums">{formatEstimatedCost(item.estimated_cost_usd_micros)}</TableTd>
                <TableTd className="text-xs whitespace-nowrap text-(--color-text-muted)">{new Date(item.last_used_at).toLocaleString()}</TableTd>
                <TableTd>
                  <div className="flex justify-end gap-1">
                    <Link
                      to="/app/resources/$kind/$resourceId"
                      params={{ kind: item.kind, resourceId: item.resource_id }}
                      className={cn(buttonVariants({ variant: "ghost", size: "icon" }), "size-8")}
                      aria-label={`Open ${item.name}`}
                    ><ArrowUpRight className="size-3.5" /></Link>
                    <a
                      href={usageHref({ resource_id: item.resource_id, version_id: item.version_id })}
                      className={cn(buttonVariants({ variant: "outline", size: "sm" }), "h-8")}
                    ><Eye className="size-3.5" />Activity</a>
                  </div>
                </TableTd>
              </TableRow>
            )
          })}
        </TableBody>
      </Table>
    </TableWrap>
  )
}

export function ResourceMemberBreakdownTable({ items }: { items: ResourceUsageMember[] }) {
  return (
    <TableWrap className="rounded-none border-0">
      <Table>
        <TableHead><tr><TableTh>Member</TableTh><TableTh>Current role</TableTh><TableTh>Requests</TableTh><TableTh>Resource uses</TableTh><TableTh>Tokens</TableTh><TableTh>Est. cost</TableTh><TableTh><span className="sr-only">Actions</span></TableTh></tr></TableHead>
        <TableBody>
          {items.map((item) => (
            <TableRow key={item.user_id}>
              <TableTd><Link to="/app/members/$userId" params={{ userId: item.user_id }} className="font-medium hover:text-(--color-accent)">{item.display_name}</Link><div className="text-xs text-(--color-text-subtle)">{item.email}</div></TableTd>
              <TableTd><Badge tone="accent">{PRIMARY_ROLE_LABELS[item.primary_role]}</Badge></TableTd>
              <TableTd className="tabular-nums">{item.requests.toLocaleString()}</TableTd>
              <TableTd className="tabular-nums">{item.resource_uses.toLocaleString()}</TableTd>
              <TableTd className="font-medium tabular-nums">{formatTokens(item.total_tokens)}</TableTd>
              <TableTd className="tabular-nums">{formatEstimatedCost(item.estimated_cost_usd_micros)}</TableTd>
              <TableTd><a href={usageHref({ member_id: item.user_id })} className={cn(buttonVariants({ variant: "outline", size: "sm" }), "h-8")}><Eye className="size-3.5" />Activity</a></TableTd>
            </TableRow>
          ))}
        </TableBody>
      </Table>
    </TableWrap>
  )
}

export function ResourceModelBreakdownTable({ items }: { items: ResourceUsageModel[] }) {
  return (
    <TableWrap className="rounded-none border-0">
      <Table>
        <TableHead><tr><TableTh>Provider / model</TableTh><TableTh>Calls</TableTh><TableTh>Tokens</TableTh><TableTh>Est. cost</TableTh><TableTh>Pricing coverage</TableTh><TableTh><span className="sr-only">Actions</span></TableTh></tr></TableHead>
        <TableBody>
          {items.map((item) => (
            <TableRow key={`${item.provider}:${item.model}`}>
              <TableTd><div className="flex items-center gap-2"><ProviderBrandIcon providerId={item.provider} /><div><div className="font-medium">{item.model}</div><div className="text-xs text-(--color-text-subtle)">{item.provider}</div></div></div></TableTd>
              <TableTd className="tabular-nums">{item.calls.toLocaleString()}</TableTd>
              <TableTd className="font-medium tabular-nums">{formatTokens(item.total_tokens)}</TableTd>
              <TableTd className="tabular-nums">{formatEstimatedCost(item.estimated_cost_usd_micros)}</TableTd>
              <TableTd>{item.unpriced_calls > 0 ? <Badge tone="warning">{item.unpriced_calls} unpriced</Badge> : <Badge tone="success">Fully priced</Badge>}</TableTd>
              <TableTd><a href={usageHref({ provider: item.provider, model: item.model })} className={cn(buttonVariants({ variant: "outline", size: "sm" }), "h-8")}><Eye className="size-3.5" />Activity</a></TableTd>
            </TableRow>
          ))}
        </TableBody>
      </Table>
    </TableWrap>
  )
}

export function ResourceRoleBreakdownTable({ items }: { items: ResourceUsageRole[] }) {
  return (
    <TableWrap className="rounded-none border-0">
      <Table>
        <TableHead><tr><TableTh>Recorded role</TableTh><TableTh>Requests</TableTh><TableTh>Model calls</TableTh><TableTh>Tool calls</TableTh><TableTh>Tokens</TableTh><TableTh>Est. cost</TableTh></tr></TableHead>
        <TableBody>
          {items.map((item) => (
            <TableRow key={item.primary_role}>
              <TableTd><Badge tone="accent">{PRIMARY_ROLE_LABELS[item.primary_role]}</Badge></TableTd>
              <TableTd className="tabular-nums">{item.requests.toLocaleString()}</TableTd>
              <TableTd className="tabular-nums">{item.model_calls.toLocaleString()}</TableTd>
              <TableTd className="tabular-nums">{item.tool_calls.toLocaleString()}</TableTd>
              <TableTd className="font-medium tabular-nums">{formatTokens(item.total_tokens)}</TableTd>
              <TableTd className="tabular-nums">{formatEstimatedCost(item.estimated_cost_usd_micros)}</TableTd>
            </TableRow>
          ))}
        </TableBody>
      </Table>
    </TableWrap>
  )
}

export function ResourceToolBreakdownTable({ items }: { items: ResourceUsageTool[] }) {
  return (
    <TableWrap className="rounded-none border-0">
      <Table>
        <TableHead><tr><TableTh>Tool</TableTh><TableTh>Category</TableTh><TableTh>Calls</TableTh><TableTh>Outcome</TableTh><TableTh>Avg duration</TableTh><TableTh>Last used</TableTh><TableTh><span className="sr-only">Actions</span></TableTh></tr></TableHead>
        <TableBody>
          {items.map((item) => (
            <TableRow key={`${item.category}:${item.tool_name}`}>
              <TableTd className="font-medium">{item.tool_name}</TableTd>
              <TableTd><Badge tone="neutral" className="capitalize">{item.category.replaceAll("_", " ")}</Badge></TableTd>
              <TableTd className="font-medium tabular-nums">{item.calls.toLocaleString()}</TableTd>
              <TableTd className="tabular-nums"><div>{item.successes} success · {item.errors} error</div><div className="text-xs text-(--color-text-subtle)">{item.blocked} blocked · {item.cancelled} cancelled</div></TableTd>
              <TableTd className="tabular-nums">{formatDuration(item.average_duration_ms)}</TableTd>
              <TableTd className="text-xs whitespace-nowrap text-(--color-text-muted)">{new Date(item.last_used_at).toLocaleString()}</TableTd>
              <TableTd><a href={usageHref({ tool_name: item.tool_name })} className={cn(buttonVariants({ variant: "outline", size: "sm" }), "h-8")}><Eye className="size-3.5" />Activity</a></TableTd>
            </TableRow>
          ))}
        </TableBody>
      </Table>
    </TableWrap>
  )
}

function usageHref(filters: Record<string, string>) {
  const search = new URLSearchParams(window.location.search)
  Object.entries(filters).forEach(([key, value]) => search.set(key, value))
  const suffix = search.toString()
  return `${RESOURCE_USAGE_PATHS.activity}${suffix ? `?${suffix}` : ""}`
}
