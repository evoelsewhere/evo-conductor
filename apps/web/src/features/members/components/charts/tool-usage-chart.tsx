import { Bar, BarChart, CartesianGrid, XAxis, YAxis } from "recharts"

import { formatNumber } from "@/features/members/components/usage-formatters"
import type { MemberToolUsage } from "@/shared/api/client"
import { ChartCard } from "@/shared/components/chart-card"
import { TELEMETRY_TOP_TOOLS_LIMIT } from "@/shared/constants/telemetry"
import {
  AccessibleChartTable,
  ChartContainer,
  ChartTooltip,
  ChartTooltipContent,
  type ChartConfig,
} from "@/shared/ui/chart"

const TOOL_USAGE_CONFIG = {
  calls: {
    label: "Calls",
    color: "var(--chart-series-1)",
  },
} satisfies ChartConfig

const TOOL_CHART_MIN_HEIGHT = 240
const TOOL_CHART_ROW_HEIGHT = 34
const TOOL_CHART_MARGIN = { top: 4, right: 12, bottom: 0, left: 0 } as const
const TOOL_AXIS_WIDTH = 104
const TOOL_NAME_MAX_LENGTH = 16

export function ToolUsageChart({ tools }: { tools: MemberToolUsage[] }) {
  const chartData = tools.slice(0, TELEMETRY_TOP_TOOLS_LIMIT)
  const height = Math.max(TOOL_CHART_MIN_HEIGHT, chartData.length * TOOL_CHART_ROW_HEIGHT)

  return (
    <ChartCard title="Most used tools" description="Calls by tool in the selected range.">
      {chartData.length === 0 ? (
        <div className="grid h-60 place-items-center text-sm text-(--color-text-muted)">
          No tool calls in this range.
        </div>
      ) : (
        <>
          <ChartContainer config={TOOL_USAGE_CONFIG} className="w-full" style={{ height }}>
            <BarChart
              accessibilityLayer
              data={chartData}
              layout="vertical"
              margin={TOOL_CHART_MARGIN}
            >
              <CartesianGrid horizontal={false} stroke="var(--border-soft)" strokeDasharray="3 3" />
              <XAxis
                type="number"
                axisLine={false}
                tickLine={false}
                tick={{ fill: "var(--color-text-subtle)", fontSize: 10 }}
                allowDecimals={false}
              />
              <YAxis
                dataKey="tool_name"
                type="category"
                axisLine={false}
                tickLine={false}
                width={TOOL_AXIS_WIDTH}
                tickFormatter={truncateToolName}
                tick={{ fill: "var(--color-text-muted)", fontSize: 10 }}
              />
              <ChartTooltip
                cursor={{ fill: "var(--color-accent-soft)" }}
                content={
                  <ChartTooltipContent
                    config={TOOL_USAGE_CONFIG}
                    valueFormatter={(value) => formatNumber(Number(value))}
                  />
                }
              />
              <Bar
                dataKey="calls"
                fill="var(--color-calls)"
                radius={[0, 5, 5, 0]}
              />
            </BarChart>
          </ChartContainer>
          <AccessibleChartTable
            caption="Most used tools"
            rows={chartData.map(({ tool_name, category, calls, successes, errors }) => ({
              tool_name,
              category,
              calls,
              successes,
              errors,
            }))}
            columns={[
              { key: "tool_name", label: "Tool" },
              { key: "category", label: "Category" },
              { key: "calls", label: "Calls" },
              { key: "successes", label: "Successful calls" },
              { key: "errors", label: "Failed calls" },
            ]}
          />
        </>
      )}
    </ChartCard>
  )
}

function truncateToolName(value: string) {
  return value.length > TOOL_NAME_MAX_LENGTH
    ? `${value.slice(0, TOOL_NAME_MAX_LENGTH)}…`
    : value
}
