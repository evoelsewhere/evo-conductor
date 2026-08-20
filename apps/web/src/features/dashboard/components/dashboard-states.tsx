import { Activity } from "lucide-react"

import type { ResourceUsageScope } from "@/shared/api/client"
import { Button, buttonVariants } from "@/shared/ui/button"
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/shared/ui/card"
import { EmptyState, ErrorState } from "@/shared/ui/empty-state"
import { LoadingState, Skeleton } from "@/shared/ui/skeleton"

export function TelemetryReadiness({
  hasConnections,
  allRequests,
  analyticsHref,
  scope,
  className,
}: {
  hasConnections: boolean
  allRequests: number
  analyticsHref: string
  scope: ResourceUsageScope
  className?: string
}) {
  return (
    <EmptyState
      icon={Activity}
      title={scope === "all" ? "No EvoFlux activity in this range" : "No governed activity in this range"}
      description={
        scope === "all"
          ? hasConnections
            ? "EvoFlux is connected, but Conductor has not received project telemetry in this range."
            : "No member is connected right now. Connect EvoFlux to populate project monitoring."
          : allRequests > 0
          ? `${allRequests.toLocaleString()} EvoFlux requests were received, but none used an Agent, Skill or Plugin governed by Conductor.`
          : hasConnections
          ? "EvoFlux is connected, but no Agent, Skill or Plugin usage was attributed during this range."
          : "No member is connected right now. Connect EvoFlux and use a governed resource to populate monitoring."
      }
      className={className}
      action={
        <a
          href={analyticsHref}
          className={buttonVariants({ variant: "outline", size: "sm" })}
        >
          Open governed resource monitoring
        </a>
      }
    />
  )
}

export function GettingStarted({
  canManageMembers,
}: {
  canManageMembers: boolean
}) {
  const steps = [
    ...(canManageMembers
      ? [
          {
            title: "Add your team",
            description: "Invite or approve members and assign project roles.",
            href: "/app/members",
          },
        ]
      : []),
    {
      title: "Create a connection token",
      description: "Generate your own evc_ token and store it securely in EvoFlux.",
      href: "/app/secrets",
    },
    {
      title: "Publish a governed resource",
      description: "Add an Agent, Skill or Plugin to the project catalog.",
      href: "/app/resources",
    },
  ]

  return (
    <Card>
      <CardHeader>
        <div>
          <CardTitle>Getting started</CardTitle>
          <CardDescription className="mt-0.5">
            Complete the project path once; monitoring replaces this checklist as activity arrives.
          </CardDescription>
        </div>
      </CardHeader>
      <CardContent className="grid gap-3 md:grid-cols-3">
        {steps.map((step, index) => (
          <a
            key={step.href}
            href={step.href}
            className="group flex gap-3 rounded-lg border border-(--border-soft) bg-(--bg-key)/25 p-3 outline-none transition-colors hover:border-(--color-border-strong) focus-visible:ring-2 focus-visible:ring-(--focus-ring)/35"
          >
            <span className="grid size-7 shrink-0 place-items-center rounded-full bg-(--color-accent-soft) text-xs font-semibold text-(--color-accent)">
              {index + 1}
            </span>
            <span className="min-w-0">
              <span className="block text-xs font-medium">{step.title}</span>
              <span className="mt-0.5 block text-[0.68rem] leading-relaxed text-(--color-text-subtle)">
                {step.description}
              </span>
            </span>
          </a>
        ))}
      </CardContent>
    </Card>
  )
}

export function PartialErrorPanel({
  summaryError,
  analyticsError,
  pendingError,
  onRetry,
}: {
  summaryError: Error | null
  analyticsError: Error | null
  pendingError: Error | null
  onRetry: () => void
}) {
  const messages = [
    summaryError && `Project snapshot: ${summaryError.message}`,
    analyticsError && `Governed analytics: ${analyticsError.message}`,
    pendingError && `Pending approvals: ${pendingError.message}`,
  ].filter((value): value is string => Boolean(value))

  return (
    <Card>
      <CardContent className="flex flex-col gap-3 p-3 sm:flex-row sm:items-center">
        <ErrorState className="min-w-0 flex-1" message={messages.join(" ")} />
        <Button variant="outline" size="sm" onClick={onRetry}>
          Retry failed data
        </Button>
      </CardContent>
    </Card>
  )
}

export function DashboardSkeleton() {
  return (
    <LoadingState label="Loading dashboard" className="grid gap-4">
      <div className="grid overflow-hidden rounded-xl border border-(--border-card) bg-(--bg-card) sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-6">
        {Array.from({ length: 6 }, (_, index) => (
          <div key={index} className="border-b border-r border-(--border-soft) px-4 py-3">
            <Skeleton className="h-3 w-24" />
            <Skeleton className="mt-2 h-7 w-20" />
            <Skeleton className="mt-1.5 h-3 w-32 max-w-full" />
          </div>
        ))}
        <div className="col-span-full border-t border-(--border-soft) px-4 py-2.5">
          <Skeleton className="h-3 w-4/5 max-w-xl" />
        </div>
      </div>
      <div className="grid items-start gap-4 xl:grid-cols-12">
        <div className="min-w-0 xl:col-span-8" style={{ alignSelf: "start" }}>
          <DashboardPanelSkeleton variant="chart" />
        </div>
        <DashboardPanelSkeleton className="xl:col-span-4" variant="operations" />
      </div>
      <DashboardPanelSkeleton variant="member" />
      <div className="grid items-start gap-4 xl:grid-cols-12">
        <div className="min-w-0 xl:col-span-8" style={{ alignSelf: "start" }}>
          <DashboardPanelSkeleton variant="signals" />
        </div>
        <DashboardPanelSkeleton className="xl:col-span-4" variant="workspace" />
      </div>
    </LoadingState>
  )
}

export function ChartSkeletons({ announce = true }: { announce?: boolean }) {
  return (
    <LoadingState
      label="Loading governed analytics"
      announce={announce}
      className="grid gap-4 md:grid-cols-2"
    >
      <DashboardChartSkeleton />
      <DashboardChartSkeleton />
    </LoadingState>
  )
}

function DashboardChartSkeleton() {
  return (
    <div className="rounded-xl border border-(--border-card) bg-(--bg-card) p-4">
      <Skeleton className="h-4 w-32" />
      <Skeleton className="mt-2 h-3 w-52 max-w-full" />
      <div className="mt-5 grid h-52 grid-cols-8 items-end gap-2 border-b border-l border-(--border-soft) px-3 pb-3">
        {[42, 68, 51, 82, 63, 74, 58, 88].map((height, index) => (
          <Skeleton
            key={index}
            className="w-full rounded-b-none"
            style={{ height: `${height}%` }}
          />
        ))}
      </div>
      <div className="mt-3 flex justify-center gap-4">
        <Skeleton className="h-3 w-16" />
        <Skeleton className="h-3 w-20" />
      </div>
    </div>
  )
}

function DashboardPanelSkeleton({
  className,
  variant,
}: {
  className?: string
  variant: "chart" | "operations" | "member" | "signals" | "workspace"
}) {
  if (variant === "chart") {
    return (
      <div
        data-dashboard-skeleton-panel={variant}
        className={`grid gap-4 md:grid-cols-2 ${className ?? ""}`}
      >
        <DashboardChartSkeleton />
        <DashboardChartSkeleton />
      </div>
    )
  }

  const rows =
    variant === "operations"
      ? 7
      : variant === "member"
        ? 3
        : variant === "signals"
          ? 4
          : 6
  return (
    <div
      data-dashboard-skeleton-panel={variant}
      className={`rounded-xl border border-(--border-card) bg-(--bg-card) p-4 ${className ?? ""}`}
    >
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0 flex-1">
          <Skeleton className="h-4 w-28" />
          <Skeleton className="mt-2 h-3 w-56 max-w-full" />
        </div>
        <Skeleton className="h-6 w-20" />
      </div>
      <div
        className={
          variant === "signals"
            ? "mt-5 grid gap-5 sm:grid-cols-2 2xl:grid-cols-4"
            : "mt-5 grid gap-3"
        }
      >
        {Array.from({ length: rows }, (_, index) => (
          <div key={index} className="grid gap-2">
            <Skeleton className="h-3 w-24" />
            <Skeleton className="h-8 w-full" />
          </div>
        ))}
      </div>
    </div>
  )
}
