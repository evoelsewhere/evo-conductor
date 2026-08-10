import { Area, AreaChart, CartesianGrid, XAxis, YAxis } from "recharts"

import { formatNumber, formatShortDate, formatTokens } from "@/features/members/components/usage-formatters"
import type { DailyTokenUsage } from "@/shared/api/client"
import { ChartCard } from "@/shared/components/chart-card"
import {
  AccessibleChartTable,
  ChartContainer,
  ChartLegendList,
  ChartTooltip,
  ChartTooltipContent,
  type ChartConfig,
} from "@/shared/ui/chart"

const TOKEN_TREND_CONFIG = {
  tokens_in: {
    label: "Input",
    color: "var(--chart-series-1)",
  },
  tokens_out: {
    label: "Output",
    color: "var(--chart-series-2)",
  },
} satisfies ChartConfig

const TOKEN_TREND_MARGIN = { top: 8, right: 8, bottom: 0, left: 0 } as const

export function TokenTrendChart({ daily }: { daily: DailyTokenUsage[] }) {
  return (
    <ChartCard title="Token trend" description="Daily input and output totals.">
      {daily.length === 0 ? (
        <div className="grid h-60 place-items-center text-sm text-(--color-text-muted)">
          No usage in this range.
        </div>
      ) : (
        <>
          <ChartContainer config={TOKEN_TREND_CONFIG} className="h-60 w-full">
            <AreaChart accessibilityLayer data={daily} margin={TOKEN_TREND_MARGIN}>
              <defs>
                <linearGradient id="token-input-fill" x1="0" y1="0" x2="0" y2="1">
                  <stop offset="5%" stopColor="var(--color-tokens_in)" stopOpacity={0.3} />
                  <stop offset="95%" stopColor="var(--color-tokens_in)" stopOpacity={0.02} />
                </linearGradient>
                <linearGradient id="token-output-fill" x1="0" y1="0" x2="0" y2="1">
                  <stop offset="5%" stopColor="var(--color-tokens_out)" stopOpacity={0.3} />
                  <stop offset="95%" stopColor="var(--color-tokens_out)" stopOpacity={0.02} />
                </linearGradient>
              </defs>
              <CartesianGrid vertical={false} stroke="var(--border-soft)" strokeDasharray="3 3" />
              <XAxis
                dataKey="date"
                axisLine={false}
                tickLine={false}
                minTickGap={28}
                tickFormatter={formatShortDate}
                tick={{ fill: "var(--color-text-subtle)", fontSize: 10 }}
              />
              <YAxis
                axisLine={false}
                tickLine={false}
                width={42}
                tickFormatter={formatTokens}
                tick={{ fill: "var(--color-text-subtle)", fontSize: 10 }}
              />
              <ChartTooltip
                cursor={{ stroke: "var(--color-border-strong)", strokeDasharray: "3 3" }}
                content={
                  <ChartTooltipContent
                    config={TOKEN_TREND_CONFIG}
                    valueFormatter={(value) => formatNumber(Number(value))}
                  />
                }
              />
              <Area
                dataKey="tokens_in"
                type="monotone"
                fill="url(#token-input-fill)"
                stroke="var(--color-tokens_in)"
                strokeWidth={2}
                stackId="tokens"
              />
              <Area
                dataKey="tokens_out"
                type="monotone"
                fill="url(#token-output-fill)"
                stroke="var(--color-tokens_out)"
                strokeWidth={2}
                stackId="tokens"
              />
            </AreaChart>
          </ChartContainer>
          <ChartLegendList
            className="mt-3 flex flex-wrap justify-center gap-x-5"
            items={Object.entries(TOKEN_TREND_CONFIG).map(([key, item]) => ({
              key,
              label: item.label,
              color: item.color,
            }))}
          />
          <AccessibleChartTable
            caption="Daily token usage"
            rows={daily.map((item) => ({ ...item }))}
            columns={[
              { key: "date", label: "Date" },
              { key: "requests", label: "Requests" },
              { key: "tokens_in", label: "Input tokens" },
              { key: "tokens_out", label: "Output tokens" },
              { key: "total_tokens", label: "Total tokens" },
            ]}
          />
        </>
      )}
    </ChartCard>
  )
}
