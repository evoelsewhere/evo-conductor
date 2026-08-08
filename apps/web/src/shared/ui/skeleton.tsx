import type { ComponentProps } from "react"

import { cn } from "@/shared/lib/utils"

function Skeleton({ className, ...props }: ComponentProps<"div">) {
  return (
    <div
      data-slot="skeleton"
      aria-hidden="true"
      className={cn("animate-pulse rounded-md bg-(--bg-key)", className)}
      {...props}
    />
  )
}

/** Placeholder rows matching the table density, to avoid layout shift. */
function SkeletonRows({
  rows = 4,
  className,
}: {
  rows?: number
  className?: string
}) {
  return (
    <div className={cn("divide-y divide-(--border-soft)", className)}>
      {Array.from({ length: rows }, (_, i) => (
        <div key={i} className="flex items-center gap-3 px-4 py-3.5">
          <Skeleton className="h-3.5 flex-1" style={{ maxWidth: `${52 - i * 6}%` }} />
          <Skeleton className="h-3.5 w-16" />
        </div>
      ))}
    </div>
  )
}

export { Skeleton, SkeletonRows }
