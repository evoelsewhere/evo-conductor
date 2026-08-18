import type { ComponentProps, ReactNode } from "react"

import { cn } from "@/shared/lib/utils"

function Skeleton({ className, ...props }: ComponentProps<"div">) {
  return (
    <div
      data-slot="skeleton"
      aria-hidden="true"
      className={cn("skeleton-shimmer rounded-md", className)}
      {...props}
    />
  )
}

function LoadingState({
  label,
  announce = true,
  className,
  children,
  ...props
}: Omit<ComponentProps<"div">, "children"> & {
  label: string
  announce?: boolean
  children: ReactNode
}) {
  return (
    <div
      {...props}
      data-slot="loading-state"
      role={announce ? "status" : undefined}
      aria-live={announce ? "polite" : undefined}
      aria-atomic={announce ? "true" : undefined}
      className={className}
    >
      <span className="sr-only">{label}</span>
      <div aria-hidden="true" className="contents">
        {children}
      </div>
    </div>
  )
}

/** Placeholder rows matching the table density, to avoid layout shift. */
function SkeletonRows({
  rows = 4,
  className,
  label = "Loading rows",
}: {
  rows?: number
  className?: string
  label?: string
}) {
  const widths = [68, 56, 72, 48]

  return (
    <LoadingState label={label} className={className}>
      <div className="divide-y divide-(--border-soft)">
        {Array.from({ length: rows }, (_, index) => (
          <div key={index} className="flex items-center gap-3 px-4 py-3.5">
            <Skeleton
              className="h-3.5 flex-1"
              style={{ maxWidth: `${widths[index % widths.length]}%` }}
            />
            <Skeleton className="h-3.5 w-16" />
          </div>
        ))}
      </div>
    </LoadingState>
  )
}

export { LoadingState, Skeleton, SkeletonRows }
