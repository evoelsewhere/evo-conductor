import type { ComponentProps } from "react"

import { cn } from "@/shared/lib/utils"
import { Badge } from "@/shared/ui/badge"
import { Tooltip } from "@/shared/ui/tooltip"

/**
 * Renders a bounded row of badges. Anything past `max` collapses into a
 * `+N` chip whose tooltip lists the remainder, so a row's height stays
 * predictable no matter how many roles or tags a record carries.
 */
function BadgeList({
  items,
  max = 3,
  tone,
  emptyLabel = "—",
  className,
}: {
  items: readonly string[]
  max?: number
  tone?: ComponentProps<typeof Badge>["tone"]
  emptyLabel?: string
  className?: string
}) {
  if (items.length === 0) {
    return <span className="text-xs text-(--color-text-subtle)">{emptyLabel}</span>
  }

  const visible = items.length > max ? items.slice(0, max) : items
  const hidden = items.slice(visible.length)

  return (
    <div className={cn("flex flex-wrap items-center gap-1", className)}>
      {visible.map((label) => (
        <Badge key={label} tone={tone} className="max-w-32 truncate">
          {label}
        </Badge>
      ))}
      {hidden.length > 0 && (
        <Tooltip side="top" content={hidden.join(", ")}>
          <span>
            <Badge tone={tone} className="cursor-default tabular-nums">
              +{hidden.length}
            </Badge>
          </span>
        </Tooltip>
      )}
    </div>
  )
}

export { BadgeList }
