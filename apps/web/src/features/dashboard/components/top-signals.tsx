import { Link } from "@tanstack/react-router"
import { Activity, ArrowRight, Boxes } from "lucide-react"
import type { ReactNode } from "react"

import { DASHBOARD_TOP_SIGNAL_LIMIT } from "@/features/dashboard/lib/dashboard-config"
import { dashboardInitials } from "@/features/dashboard/lib/dashboard-formatters"
import {
  formatDuration,
  formatTokens,
} from "@/features/members/components/usage-formatters"
import type {
  ResourceUsageBreakdown,
  ResourceUsageMember,
  ResourceUsageModel,
  ResourceUsageTool,
} from "@/shared/api/client"
import { ProviderBrandIcon } from "@/shared/components/provider-brand-icon"
import { RESOURCE_KIND_LABEL } from "@/shared/constants/resource"
import { cn } from "@/shared/lib/utils"
import { buttonVariants } from "@/shared/ui/button"
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/shared/ui/card"
import { LoadingState, Skeleton } from "@/shared/ui/skeleton"

export function TopSignals({
  members,
  resources,
  models,
  tools,
  showMembers,
  loading,
  analyticsHref,
  className,
  announceLoading = true,
}: {
  members: ResourceUsageMember[]
  resources: ResourceUsageBreakdown[]
  models: ResourceUsageModel[]
  tools: ResourceUsageTool[]
  showMembers: boolean
  loading: boolean
  analyticsHref: (filters?: Record<string, string>) => string
  className?: string
  announceLoading?: boolean
}) {
  return (
    <Card className={className}>
      <CardHeader>
        <div>
          <CardTitle>Top signals</CardTitle>
          <CardDescription className="mt-0.5">
            Operational volume for the selected range, not a performance score.
          </CardDescription>
        </div>
        <a
          href={analyticsHref()}
          className={buttonVariants({ variant: "outline", size: "sm" })}
        >
          Open Analytics
          <ArrowRight className="size-3.5" />
        </a>
      </CardHeader>
      <CardContent
        className={cn(
          "grid gap-5",
          showMembers
            ? "sm:grid-cols-2 2xl:grid-cols-4"
            : "sm:grid-cols-2 2xl:grid-cols-3",
        )}
      >
        {loading ? (
          <LoadingState
            label="Loading top signals"
            announce={announceLoading}
            className={cn(
              "col-span-full grid gap-5",
              showMembers
                ? "sm:grid-cols-2 2xl:grid-cols-4"
                : "sm:grid-cols-2 2xl:grid-cols-3",
            )}
          >
            {Array.from({ length: showMembers ? 4 : 3 }, (_, index) => (
              <SignalSkeleton key={index} />
            ))}
          </LoadingState>
        ) : (
          <>
            {showMembers && <MemberSignals items={members} />}
            <ResourceSignals items={resources} analyticsHref={analyticsHref} />
            <ModelSignals items={models} analyticsHref={analyticsHref} />
            <ToolSignals items={tools} analyticsHref={analyticsHref} />
          </>
        )}
      </CardContent>
    </Card>
  )
}

function SignalSkeleton() {
  return (
    <div className="grid gap-2">
      <Skeleton className="h-3 w-20" />
      <Skeleton className="h-9 w-full" />
      <Skeleton className="h-9 w-full" />
      <Skeleton className="h-9 w-full" />
    </div>
  )
}

function MemberSignals({ items }: { items: ResourceUsageMember[] }) {
  return (
    <SignalSection
      id="dashboard-members"
      title="Top members"
      description="By governed requests"
      empty="No member activity"
      hasItems={items.length > 0}
    >
      {items.slice(0, DASHBOARD_TOP_SIGNAL_LIMIT).map((item) => (
        <Link
          key={item.user_id}
          to="/app/members/$userId"
          params={{ userId: item.user_id }}
          className="group flex items-center gap-2 py-2 outline-none focus-visible:ring-2 focus-visible:ring-(--focus-ring)/35"
        >
          <span className="grid size-7 shrink-0 place-items-center rounded-full bg-(--color-accent-soft) text-[0.68rem] font-semibold text-(--color-accent)">
            {dashboardInitials(item.display_name)}
          </span>
          <span className="min-w-0 flex-1">
            <span className="block truncate text-xs font-medium">
              {item.display_name}
            </span>
            <span className="block text-[0.68rem] text-(--color-text-subtle)">
              {item.resource_uses.toLocaleString()} resource uses
            </span>
          </span>
          <span className="text-right text-xs font-semibold tabular-nums">
            {item.requests.toLocaleString()}
            <span className="block text-[0.62rem] font-normal text-(--color-text-subtle)">
              requests
            </span>
          </span>
        </Link>
      ))}
    </SignalSection>
  )
}

function ResourceSignals({
  items,
  analyticsHref,
}: {
  items: ResourceUsageBreakdown[]
  analyticsHref: (filters?: Record<string, string>) => string
}) {
  return (
    <SignalSection
      id="dashboard-resources"
      title="Resources"
      description="Top attributed versions"
      empty="No resource activity"
      hasItems={items.length > 0}
    >
      {items.slice(0, DASHBOARD_TOP_SIGNAL_LIMIT).map((item) => (
        <a
          key={`${item.resource_id}:${item.version_id}:${item.relation}`}
          href={analyticsHref({ resource_id: item.resource_id })}
          className="group flex items-center gap-2 py-2 outline-none focus-visible:ring-2 focus-visible:ring-(--focus-ring)/35"
        >
          <span className="grid size-7 shrink-0 place-items-center rounded-md bg-(--bg-key) text-(--color-text-subtle)">
            <Boxes className="size-3.5" />
          </span>
          <span className="min-w-0 flex-1">
            <span className="block truncate text-xs font-medium">
              {item.name}
            </span>
            <span className="block truncate text-[0.68rem] text-(--color-text-subtle)">
              {RESOURCE_KIND_LABEL[item.kind]} · v{item.version}
            </span>
          </span>
          <span className="text-right text-xs font-semibold tabular-nums">
            {item.uses.toLocaleString()}
            <span className="block text-[0.62rem] font-normal text-(--color-text-subtle)">
              uses
            </span>
          </span>
        </a>
      ))}
    </SignalSection>
  )
}

function ModelSignals({
  items,
  analyticsHref,
}: {
  items: ResourceUsageModel[]
  analyticsHref: (filters?: Record<string, string>) => string
}) {
  return (
    <SignalSection
      id="dashboard-models"
      title="Models"
      description="Calls and estimate coverage"
      empty="No model activity"
      hasItems={items.length > 0}
    >
      {items.slice(0, DASHBOARD_TOP_SIGNAL_LIMIT).map((item) => (
        <a
          key={`${item.provider}:${item.model}`}
          href={analyticsHref({ provider: item.provider, model: item.model })}
          className="group flex items-center gap-2 py-2 outline-none focus-visible:ring-2 focus-visible:ring-(--focus-ring)/35"
        >
          <ProviderBrandIcon
            providerId={item.provider}
            className="size-6 shrink-0"
          />
          <span className="min-w-0 flex-1">
            <span className="block truncate text-xs font-medium">
              {item.model}
            </span>
            <span className="block truncate text-[0.68rem] text-(--color-text-subtle)">
              {formatTokens(item.total_tokens)} tokens
            </span>
          </span>
          <span className="text-right text-xs font-semibold tabular-nums">
            {item.calls.toLocaleString()}
            <span
              className={cn(
                "block text-[0.62rem] font-normal",
                item.unpriced_calls > 0
                  ? "text-(--color-warning)"
                  : "text-(--color-text-subtle)",
              )}
            >
              {item.unpriced_calls > 0
                ? `${item.unpriced_calls} unpriced`
                : "priced"}
            </span>
          </span>
        </a>
      ))}
    </SignalSection>
  )
}

function ToolSignals({
  items,
  analyticsHref,
}: {
  items: ResourceUsageTool[]
  analyticsHref: (filters?: Record<string, string>) => string
}) {
  return (
    <SignalSection
      id="dashboard-tools"
      title="Tools"
      description="Calls and outcomes"
      empty="No tool activity"
      hasItems={items.length > 0}
    >
      {items.slice(0, DASHBOARD_TOP_SIGNAL_LIMIT).map((item) => (
        <a
          key={`${item.category}:${item.tool_name}`}
          href={analyticsHref({ tool_name: item.tool_name })}
          className="group flex items-center gap-2 py-2 outline-none focus-visible:ring-2 focus-visible:ring-(--focus-ring)/35"
        >
          <span className="grid size-7 shrink-0 place-items-center rounded-md bg-(--bg-key) text-(--color-text-subtle)">
            <Activity className="size-3.5" />
          </span>
          <span className="min-w-0 flex-1">
            <span className="block truncate text-xs font-medium">
              {item.tool_name}
            </span>
            <span className="block truncate text-[0.68rem] text-(--color-text-subtle)">
              {formatDuration(item.average_duration_ms)} avg
            </span>
          </span>
          <span className="text-right text-xs font-semibold tabular-nums">
            {item.calls.toLocaleString()}
            <span
              className={cn(
                "block text-[0.62rem] font-normal",
                item.errors + item.blocked > 0
                  ? "text-(--color-warning)"
                  : "text-(--color-text-subtle)",
              )}
            >
              {item.errors + item.blocked} issues
            </span>
          </span>
        </a>
      ))}
    </SignalSection>
  )
}

function SignalSection({
  id,
  title,
  description,
  empty,
  hasItems,
  children,
}: {
  id: string
  title: string
  description: string
  empty: string
  hasItems: boolean
  children: ReactNode
}) {
  return (
    <section aria-labelledby={id} className="min-w-0">
      <div className="mb-1">
        <h3 id={id} className="text-xs font-semibold">
          {title}
        </h3>
        <p className="text-[0.68rem] text-(--color-text-subtle)">
          {description}
        </p>
      </div>
      {hasItems ? (
        <div className="divide-y divide-(--border-soft)">{children}</div>
      ) : (
        <div className="mt-2 rounded-lg border border-dashed border-(--border-soft) px-3 py-5 text-center text-xs text-(--color-text-subtle)">
          {empty}
        </div>
      )}
    </section>
  )
}
