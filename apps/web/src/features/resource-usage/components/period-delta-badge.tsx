import { ArrowDown, ArrowUp, Minus } from "lucide-react"

import { cn } from "@/shared/lib/utils"

/** Compares `current` against `previous` (the equal-length period right
 * before it) and renders a small colored delta — green when the change is
 * favorable, amber when it is not. Renders nothing when there is no
 * previous-period figure to compare against (comparison not requested, or
 * not yet loaded). */
export function PeriodDeltaBadge({
  current,
  previous,
  formatValue,
  higherIsBetter = true,
  className,
}: {
  current: number
  previous: number | null | undefined
  formatValue: (value: number) => string
  higherIsBetter?: boolean
  className?: string
}) {
  if (previous == null) return null
  const delta = current - previous
  if (delta === 0) {
    return (
      <span className={cn("inline-flex items-center gap-0.5 text-(--color-text-subtle)", className)}>
        <Minus className="size-3" />
        No change vs previous period
      </span>
    )
  }
  const improved = higherIsBetter ? delta > 0 : delta < 0
  const Icon = delta > 0 ? ArrowUp : ArrowDown
  return (
    <span
      className={cn(
        "inline-flex items-center gap-0.5",
        improved ? "text-(--color-success)" : "text-(--color-warning)",
        className,
      )}
    >
      <Icon className="size-3" />
      {formatValue(Math.abs(delta))} vs previous period
    </span>
  )
}
