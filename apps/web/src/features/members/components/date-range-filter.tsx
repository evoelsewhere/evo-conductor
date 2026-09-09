import { CalendarDays } from "lucide-react"
import { useMemo, useState } from "react"
import type { DateRange } from "react-day-picker"

import {
  DEFAULT_CUSTOM_RANGE_DAYS,
  DEFAULT_USAGE_RANGE_PRESET,
  MILLISECONDS_PER_DAY,
  USAGE_RANGE_DAYS,
  USAGE_RANGE_PRESET_OPTIONS,
  UsageRangePreset,
} from "@/shared/constants/telemetry"
import { Button } from "@/shared/ui/button"
import { Calendar } from "@/shared/ui/calendar"
import { Popover, PopoverContent, PopoverTrigger } from "@/shared/ui/popover"

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
  const [open, setOpen] = useState(false)
  const selectedRange = useMemo<DateRange | undefined>(() => {
    const from = parseDateInput(customFrom)
    const to = parseDateInput(customTo)
    return from || to ? { from, to } : undefined
  }, [customFrom, customTo])
  // A draft that only commits on "Apply" — react-day-picker fills in `to`
  // on the very first click (a same-day range), so treating "`to` exists"
  // as "the user is done" closes the picker before they can pick an end
  // date at all.
  const [draftRange, setDraftRange] = useState<DateRange | undefined>(selectedRange)
  const customLabel =
    preset === UsageRangePreset.Custom ? formatRangeLabel(selectedRange) : null

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
      <Popover
        open={open}
        onOpenChange={(nextOpen) => {
          setOpen(nextOpen)
          if (nextOpen) {
            onPresetChange(UsageRangePreset.Custom)
            setDraftRange(selectedRange)
          }
        }}
      >
        <PopoverTrigger
          render={
            <Button
              size="sm"
              variant={preset === UsageRangePreset.Custom ? "secondary" : "outline"}
              aria-pressed={preset === UsageRangePreset.Custom}
            >
              <CalendarDays className="size-3.5" />
              {customLabel ?? "Custom"}
            </Button>
          }
        />
        <PopoverContent className="w-auto" align="start">
          <Calendar
            mode="range"
            numberOfMonths={2}
            defaultMonth={draftRange?.from}
            selected={draftRange}
            onSelect={setDraftRange}
          />
          <div className="mt-2 flex items-center justify-between gap-2 border-t border-(--border-soft) pt-2">
            <span className="text-xs text-(--color-text-subtle)">
              {formatRangeLabel(draftRange) ?? "Pick a start and end date"}
            </span>
            <div className="flex gap-1.5">
              <Button size="sm" variant="ghost" onClick={() => setOpen(false)}>
                Cancel
              </Button>
              <Button
                size="sm"
                variant="secondary"
                disabled={!draftRange?.from || !draftRange.to}
                onClick={() => {
                  if (!draftRange?.from || !draftRange.to) return
                  onCustomFromChange(formatDateInput(draftRange.from))
                  onCustomToChange(formatDateInput(draftRange.to))
                  setOpen(false)
                }}
              >
                Apply
              </Button>
            </div>
          </div>
        </PopoverContent>
      </Popover>
    </div>
  )
}

function parseDateInput(value: string): Date | undefined {
  if (!value) return undefined
  const date = new Date(`${value}T00:00:00`)
  return Number.isNaN(date.getTime()) ? undefined : date
}

const RANGE_LABEL_FORMATTER = new Intl.DateTimeFormat(undefined, {
  month: "short",
  day: "numeric",
})
const RANGE_LABEL_FORMATTER_WITH_YEAR = new Intl.DateTimeFormat(undefined, {
  month: "short",
  day: "numeric",
  year: "numeric",
})

function formatRangeLabel(range: DateRange | undefined): string | null {
  if (!range?.from) return null
  if (!range.to || range.to.getTime() === range.from.getTime()) {
    return RANGE_LABEL_FORMATTER_WITH_YEAR.format(range.from)
  }
  const sameYear = range.from.getFullYear() === range.to.getFullYear()
  const from = sameYear
    ? RANGE_LABEL_FORMATTER.format(range.from)
    : RANGE_LABEL_FORMATTER_WITH_YEAR.format(range.from)
  return `${from} – ${RANGE_LABEL_FORMATTER_WITH_YEAR.format(range.to)}`
}

function dateInputDaysAgo(days: number) {
  return formatDateInput(new Date(Date.now() - days * MILLISECONDS_PER_DAY))
}

function formatDateInput(date: Date): string {
  const year = date.getFullYear()
  const month = String(date.getMonth() + 1).padStart(2, "0")
  const day = String(date.getDate()).padStart(2, "0")
  return `${year}-${month}-${day}`
}
