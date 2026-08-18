import type { ComponentProps } from "react"
import type { LucideIcon } from "lucide-react"

import { LoadingState, Skeleton } from "@/shared/ui/skeleton"
import { cn } from "@/shared/lib/utils"

export function StatCard({
  label,
  value,
  hint,
  icon: Icon,
  tone = "neutral",
  className,
}: {
  label: string
  value: React.ReactNode
  hint?: string
  icon?: LucideIcon
  tone?: "neutral" | "accent" | "success" | "warning"
  className?: string
}) {
  const iconTones = {
    neutral: "text-(--color-text-subtle) bg-(--bg-key)",
    accent: "text-(--color-accent) bg-(--color-accent-soft)",
    success: "text-(--color-success) bg-(--color-success)/12",
    warning: "text-(--color-warning) bg-(--color-warning)/12",
  } as const

  return (
    <div
      className={cn(
        "group rounded-xl border border-(--border-card) bg-(--bg-card) p-4 transition-colors hover:border-(--color-border-strong)",
        className,
      )}
    >
      <div className="mb-3 flex items-center justify-between gap-2">
        <span className="truncate text-xs font-medium text-(--color-text-muted)">
          {label}
        </span>
        {Icon && (
          <span
            className={cn(
              "grid size-7 shrink-0 place-items-center rounded-md",
              iconTones[tone],
            )}
          >
            <Icon className="size-3.5" strokeWidth={1.7} />
          </span>
        )}
      </div>
      <div className="text-2xl leading-none font-semibold tracking-tight tabular-nums">
        {value}
      </div>
      {hint && (
        <div className="mt-1.5 truncate text-xs text-(--color-text-subtle)">
          {hint}
        </div>
      )}
    </div>
  )
}

export function StatCardSkeleton() {
  return (
    <div className="rounded-xl border border-(--border-card) bg-(--bg-card) p-4">
      <div className="mb-3 flex items-center justify-between">
        <Skeleton className="h-3 w-20" />
        <Skeleton className="size-7" />
      </div>
      <Skeleton className="h-6 w-12" />
      <Skeleton className="mt-2 h-3 w-28" />
    </div>
  )
}

export function StatCardGrid({ className, ...props }: ComponentProps<"div">) {
  return (
    <div
      className={cn("grid gap-3 sm:grid-cols-2 lg:grid-cols-4", className)}
      {...props}
    />
  )
}

export function StatCardGridSkeleton({
  count,
  className,
  label = "Loading metrics",
  announce = true,
}: {
  count: number
  className?: string
  label?: string
  announce?: boolean
}) {
  return (
    <LoadingState label={label} announce={announce}>
      <StatCardGrid className={className}>
        {Array.from({ length: count }, (_, index) => (
          <StatCardSkeleton key={index} />
        ))}
      </StatCardGrid>
    </LoadingState>
  )
}
