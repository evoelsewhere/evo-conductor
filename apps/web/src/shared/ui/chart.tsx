import type { ComponentProps, CSSProperties, ReactNode } from "react"
import {
  ResponsiveContainer,
  Tooltip as RechartsTooltip,
  type TooltipContentProps,
  type TooltipValueType,
} from "recharts"

import { cn } from "@/shared/lib/utils"

const CHART_INITIAL_DIMENSION = { width: 320, height: 200 } as const

export interface ChartConfigItem {
  label: ReactNode
  color: string
}

export type ChartConfig = Record<string, ChartConfigItem>

type ChartStyle = CSSProperties & Record<`--color-${string}`, string>

function ChartContainer({
  config,
  className,
  style,
  children,
  ...props
}: ComponentProps<"div"> & {
  config: ChartConfig
  children: ComponentProps<typeof ResponsiveContainer>["children"]
}) {
  const chartStyle = Object.entries(config).reduce<ChartStyle>(
    (result, [key, item]) => {
      result[`--color-${key}`] = item.color
      return result
    },
    { ...style } as ChartStyle,
  )

  return (
    <div
      data-slot="chart"
      className={cn("min-h-48 min-w-0 text-xs", className)}
      style={chartStyle}
      {...props}
    >
      <ResponsiveContainer initialDimension={CHART_INITIAL_DIMENSION}>
        {children}
      </ResponsiveContainer>
    </div>
  )
}

const ChartTooltip = RechartsTooltip

type ChartTooltipContentProps = Partial<
  TooltipContentProps<TooltipValueType, string | number>
> &
  ComponentProps<"div"> & {
    config: ChartConfig
    hideLabel?: boolean
    valueFormatter?: (value: TooltipValueType) => ReactNode
  }

function ChartTooltipContent({
  active,
  payload,
  label,
  config,
  hideLabel = false,
  valueFormatter = formatChartValue,
  className,
}: ChartTooltipContentProps) {
  if (!active || !payload?.length) return null

  return (
    <div
      className={cn(
        "grid min-w-36 gap-1.5 rounded-lg border border-(--color-border) bg-(--bg-card) px-3 py-2 text-xs shadow-lg",
        className,
      )}
    >
      {!hideLabel && label != null && (
        <div className="font-medium text-(--color-text)">{String(label)}</div>
      )}
      <div className="grid gap-1.5">
        {payload.map((item, index) => {
          const key = String(item.dataKey ?? item.name ?? index)
          const configItem = config[key]
          const color = item.color ?? item.fill ?? configItem?.color
          return (
            <div key={key} className="flex items-center gap-2">
              <span
                aria-hidden="true"
                className="size-2 shrink-0 rounded-[2px]"
                style={{ backgroundColor: color }}
              />
              <span className="flex-1 text-(--color-text-muted)">
                {configItem?.label ?? item.name ?? key}
              </span>
              <span className="font-mono font-medium tabular-nums text-(--color-text)">
                {item.value == null ? "—" : valueFormatter(item.value)}
              </span>
            </div>
          )
        })}
      </div>
    </div>
  )
}

function ChartLegendList({
  items,
  className,
}: {
  items: Array<{
    key: string
    label: ReactNode
    color: string
    value?: ReactNode
    icon?: ReactNode
  }>
  className?: string
}) {
  return (
    <div className={cn("grid gap-2", className)}>
      {items.map((item) => (
        <div key={item.key} className="flex min-w-0 items-center gap-2 text-xs">
          <span
            aria-hidden="true"
            className="size-2.5 shrink-0 rounded-full"
            style={{ backgroundColor: item.color }}
          />
          {item.icon}
          <span className="min-w-0 flex-1 truncate text-(--color-text-muted)">
            {item.label}
          </span>
          {item.value != null && (
            <span className="font-mono tabular-nums text-(--color-text)">{item.value}</span>
          )}
        </div>
      ))}
    </div>
  )
}

function AccessibleChartTable({
  caption,
  columns,
  rows,
}: {
  caption: string
  columns: Array<{ key: string; label: string; format?: (value: unknown) => ReactNode }>
  rows: Array<Record<string, unknown>>
}) {
  return (
    <div className="sr-only overflow-hidden">
      <table>
        <caption>{caption}</caption>
        <thead>
          <tr>{columns.map((column) => <th key={column.key}>{column.label}</th>)}</tr>
        </thead>
        <tbody>
          {rows.map((row, rowIndex) => (
            <tr key={rowIndex}>
              {columns.map((column) => (
                <td key={column.key}>
                  {column.format?.(row[column.key]) ?? String(row[column.key] ?? "")}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  )
}

function formatChartValue(value: TooltipValueType) {
  if (Array.isArray(value)) return value.join(" – ")
  return typeof value === "number" ? new Intl.NumberFormat().format(value) : value
}

export {
  AccessibleChartTable,
  ChartContainer,
  ChartLegendList,
  ChartTooltip,
  ChartTooltipContent,
}
