import type { LucideIcon } from "lucide-react"

import { cn } from "@/shared/lib/utils"

function EmptyState({
  icon: Icon,
  title,
  description,
  action,
  className,
}: {
  icon?: LucideIcon
  title: string
  description?: string
  action?: React.ReactNode
  className?: string
}) {
  return (
    <div
      className={cn(
        "flex flex-col items-center rounded-xl border border-dashed border-(--color-border) bg-(--bg-card)/45 px-6 py-12 text-center",
        className,
      )}
    >
      {Icon && (
        <span className="mb-3 grid size-10 place-items-center rounded-lg border border-(--border-soft) bg-(--bg-key) text-(--color-text-subtle)">
          <Icon className="size-5" strokeWidth={1.6} />
        </span>
      )}
      <p className="text-sm font-medium text-(--color-text)">{title}</p>
      {description && (
        <p className="mt-1 max-w-sm text-xs leading-relaxed text-(--color-text-muted)">
          {description}
        </p>
      )}
      {action && <div className="mt-4">{action}</div>}
    </div>
  )
}

function ErrorState({
  message,
  className,
}: {
  message: string
  className?: string
}) {
  return (
    <div
      role="alert"
      className={cn(
        "rounded-lg border border-(--color-error)/30 bg-(--color-error-subtle) px-3 py-2 text-sm text-(--color-error)",
        className,
      )}
    >
      {message}
    </div>
  )
}

export { EmptyState, ErrorState }
