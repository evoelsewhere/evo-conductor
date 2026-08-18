import { Activity } from "lucide-react"

import { StatCardGridSkeleton } from "@/shared/components/stat-card"
import { Button, buttonVariants } from "@/shared/ui/button"
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/shared/ui/card"
import { EmptyState, ErrorState } from "@/shared/ui/empty-state"
import { Skeleton } from "@/shared/ui/skeleton"

export function TelemetryReadiness({
  hasConnections,
  analyticsHref,
  className,
}: {
  hasConnections: boolean
  analyticsHref: string
  className?: string
}) {
  return (
    <EmptyState
      icon={Activity}
      title="No governed activity in this range"
      description={
        hasConnections
          ? "EvoFlux is connected, but no Agent, Skill or Plugin usage was attributed during this range."
          : "No member is connected right now. Connect EvoFlux and use a governed resource to populate monitoring."
      }
      className={className}
      action={
        <a
          href={analyticsHref}
          className={buttonVariants({ variant: "outline", size: "sm" })}
        >
          Open resource monitoring
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
    <div aria-label="Loading dashboard" className="grid gap-4">
      <StatCardGridSkeleton
        count={6}
        className="lg:grid-cols-3 2xl:grid-cols-6"
      />
      <div className="grid gap-4 xl:grid-cols-12">
        <Skeleton className="h-80 xl:col-span-7" />
        <Skeleton className="h-80 xl:col-span-5" />
      </div>
      <div className="grid gap-4 xl:grid-cols-12">
        <Skeleton className="h-72 xl:col-span-9" />
        <Skeleton className="h-72 xl:col-span-3" />
      </div>
    </div>
  )
}

export function ChartSkeletons() {
  return (
    <div
      aria-label="Loading governed analytics"
      className="grid gap-4 md:grid-cols-2"
    >
      <Skeleton className="h-80" />
      <Skeleton className="h-80" />
    </div>
  )
}
