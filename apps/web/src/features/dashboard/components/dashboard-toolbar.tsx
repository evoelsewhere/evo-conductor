import { RefreshCw } from "lucide-react"

import {
  DASHBOARD_RANGE_OPTIONS,
  type DashboardRangeDays,
} from "@/features/dashboard/lib/dashboard-config"
import { formatDashboardUpdatedTime } from "@/features/dashboard/lib/dashboard-formatters"
import { cn } from "@/shared/lib/utils"
import { Button } from "@/shared/ui/button"

export function DashboardToolbar({
  rangeDays,
  isRefreshing,
  updatedAt,
  onRangeChange,
  onRefresh,
}: {
  rangeDays: DashboardRangeDays
  isRefreshing: boolean
  updatedAt: number | null
  onRangeChange: (value: DashboardRangeDays) => void
  onRefresh: () => void
}) {
  return (
    <div className="flex max-w-full flex-wrap items-center justify-end gap-2">
      <DashboardRangeFilter value={rangeDays} onChange={onRangeChange} />
      <Button
        variant="outline"
        size="sm"
        disabled={isRefreshing}
        onClick={onRefresh}
      >
        <RefreshCw
          className={cn("size-3.5", isRefreshing && "animate-spin")}
        />
        Refresh
      </Button>
      <span
        aria-live="polite"
        className="w-full text-right text-[0.68rem] text-(--color-text-subtle) sm:w-auto"
      >
        {isRefreshing
          ? "Updating…"
          : updatedAt
            ? `Updated ${formatDashboardUpdatedTime(updatedAt)}`
            : "Not updated yet"}
      </span>
    </div>
  )
}

function DashboardRangeFilter({
  value,
  onChange,
}: {
  value: DashboardRangeDays
  onChange: (value: DashboardRangeDays) => void
}) {
  return (
    <div
      role="group"
      aria-label="Dashboard time range"
      className="inline-flex rounded-md border border-(--color-border) bg-(--bg-page) p-0.5"
    >
      <span
        aria-hidden="true"
        className="flex items-center px-1.5 text-[0.65rem] text-(--color-text-subtle)"
      >
        Analytics
      </span>
      {DASHBOARD_RANGE_OPTIONS.map((days) => (
        <Button
          key={days}
          type="button"
          variant="ghost"
          size="sm"
          aria-pressed={value === days}
          className={cn(
            "h-6 px-2 text-[0.7rem]",
            value === days && "bg-(--bg-key) text-(--color-text)",
          )}
          onClick={() => onChange(days)}
        >
          {days} days
        </Button>
      ))}
    </div>
  )
}
