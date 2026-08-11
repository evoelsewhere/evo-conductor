import { CalendarDays } from "lucide-react"
import { useMemo, useState } from "react"

import {
  DEFAULT_CUSTOM_RANGE_DAYS,
  DEFAULT_USAGE_RANGE_PRESET,
  MILLISECONDS_PER_DAY,
  USAGE_RANGE_DAYS,
  USAGE_RANGE_PRESET_OPTIONS,
  UsageRangePreset,
} from "@/shared/constants/telemetry"
import { Button } from "@/shared/ui/button"
import { Input } from "@/shared/ui/input"

export function useUsageRange(
  defaultPreset: UsageRangePreset = DEFAULT_USAGE_RANGE_PRESET,
  defaultCustomFrom = dateInputDaysAgo(DEFAULT_CUSTOM_RANGE_DAYS),
  defaultCustomTo = dateInputDaysAgo(0),
) {
  const [preset, setPreset] = useState<UsageRangePreset>(defaultPreset)
  const [customFrom, setCustomFrom] = useState(defaultCustomFrom)
  const [customTo, setCustomTo] = useState(defaultCustomTo)

  const range = useMemo(() => {
    const now = new Date()
    if (preset === UsageRangePreset.Custom) {
      const from = new Date(`${customFrom}T00:00:00`)
      const to = new Date(`${customTo}T23:59:59.999`)
      return {
        from: Number.isNaN(from.getTime()) ? undefined : from.toISOString(),
        to: Number.isNaN(to.getTime()) ? undefined : to.toISOString(),
      }
    }
    const days = USAGE_RANGE_DAYS[preset]
    return {
      from: new Date(now.getTime() - days * MILLISECONDS_PER_DAY).toISOString(),
      to: now.toISOString(),
    }
  }, [customFrom, customTo, preset])

  return {
    preset,
    setPreset,
    customFrom,
    setCustomFrom,
    customTo,
    setCustomTo,
    range,
  }
}

export function DateRangeFilter({
  preset,
  onPresetChange,
  customFrom,
  onCustomFromChange,
  customTo,
  onCustomToChange,
}: {
  preset: UsageRangePreset
  onPresetChange: (preset: UsageRangePreset) => void
  customFrom: string
  onCustomFromChange: (value: string) => void
  customTo: string
  onCustomToChange: (value: string) => void
}) {
  return (
    <div className="flex flex-wrap items-center gap-2" aria-label="Usage date range">
      <div className="inline-flex rounded-md border border-(--color-border) bg-(--bg-page) p-0.5">
        {USAGE_RANGE_PRESET_OPTIONS.map((value) => (
          <Button
            key={value}
            size="sm"
            variant={preset === value ? "secondary" : "ghost"}
            className="capitalize"
            aria-pressed={preset === value}
            onClick={() => onPresetChange(value)}
          >
            {value}
          </Button>
        ))}
      </div>
      <Button
        size="sm"
        variant={preset === UsageRangePreset.Custom ? "secondary" : "outline"}
        aria-pressed={preset === UsageRangePreset.Custom}
        onClick={() => onPresetChange(UsageRangePreset.Custom)}
      >
        <CalendarDays className="size-3.5" />
        Custom
      </Button>
      {preset === UsageRangePreset.Custom && (
        <div className="flex flex-wrap items-center gap-1.5">
          <label className="flex items-center gap-1.5 text-xs text-(--color-text-muted)">
            From
            <Input
              type="date"
              className="w-36"
              value={customFrom}
              max={customTo}
              onChange={(event) => onCustomFromChange(event.target.value)}
            />
          </label>
          <label className="flex items-center gap-1.5 text-xs text-(--color-text-muted)">
            To
            <Input
              type="date"
              className="w-36"
              value={customTo}
              min={customFrom}
              onChange={(event) => onCustomToChange(event.target.value)}
            />
          </label>
        </div>
      )}
    </div>
  )
}

function dateInputDaysAgo(days: number) {
  const value = new Date(Date.now() - days * MILLISECONDS_PER_DAY)
  const year = value.getFullYear()
  const month = String(value.getMonth() + 1).padStart(2, "0")
  const date = String(value.getDate()).padStart(2, "0")
  return `${year}-${month}-${date}`
}
