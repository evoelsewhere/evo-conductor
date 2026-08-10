import { Cell, Pie, PieChart } from "recharts"

import { formatTokens } from "@/features/members/components/usage-formatters"
import type { ModelUsageBreakdown } from "@/shared/api/client"
import { ChartCard } from "@/shared/components/chart-card"
import { ProviderBrandIcon } from "@/shared/components/provider-brand-icon"
import {
  TELEMETRY_CHART_SERIES,
  TELEMETRY_PERCENT_SCALE,
} from "@/shared/constants/telemetry"
import {
  AccessibleChartTable,
  ChartContainer,
  ChartLegendList,
  ChartTooltip,
  ChartTooltipContent,
  type ChartConfig,
} from "@/shared/ui/chart"

const MODEL_TOKEN_CONFIG = {
  total_tokens: {
    label: "Tokens",
    color: "var(--chart-series-1)",
  },
} satisfies ChartConfig

const MODEL_VISIBLE_LIMIT = TELEMETRY_CHART_SERIES.length
const MODEL_PIE_RADII = { inner: 52, outer: 78 } as const

interface ModelChartItem extends ModelUsageBreakdown {
  key: string
  color: string
}

export function ModelDonutChart({ models }: { models: ModelUsageBreakdown[] }) {
  const chartData = groupModelUsage(models)
  const total = chartData.reduce((sum, item) => sum + item.total_tokens, 0)

  return (
    <ChartCard title="Tokens by model" description="Share of input and output tokens.">
      {chartData.length === 0 ? (
        <div className="grid h-60 place-items-center text-sm text-(--color-text-muted)">
          No model usage in this range.
        </div>
      ) : (
        <div className="grid gap-4 sm:grid-cols-[13rem_minmax(0,1fr)] sm:items-center">
          <div className="relative">
            <ChartContainer config={MODEL_TOKEN_CONFIG} className="h-52 w-full">
              <PieChart accessibilityLayer>
                <ChartTooltip
                  content={
                    <ChartTooltipContent
                      config={MODEL_TOKEN_CONFIG}
                      valueFormatter={(value) => formatTokens(Number(value))}
                    />
                  }
                />
                <Pie
                  data={chartData}
                  dataKey="total_tokens"
                  nameKey="model"
                  innerRadius={MODEL_PIE_RADII.inner}
                  outerRadius={MODEL_PIE_RADII.outer}
                  paddingAngle={2}
                  stroke="var(--bg-card)"
                  strokeWidth={2}
                >
                  {chartData.map((item) => <Cell key={item.key} fill={item.color} />)}
                </Pie>
              </PieChart>
            </ChartContainer>
            <div className="pointer-events-none absolute inset-0 grid place-items-center text-center">
              <div>
                <div className="text-xl font-semibold tabular-nums">{formatTokens(total)}</div>
                <div className="text-[0.65rem] text-(--color-text-subtle)">total tokens</div>
              </div>
            </div>
          </div>
          <ChartLegendList
            items={chartData.map((item) => ({
              key: item.key,
              label: (
                <span>
                  <span className="font-medium text-(--color-text)">{item.model}</span>
                  {item.provider && (
                    <span className="ml-1 text-(--color-text-subtle)">{item.provider}</span>
                  )}
                </span>
              ),
              color: item.color,
              icon: item.provider ? <ProviderBrandIcon providerId={item.provider} /> : undefined,
              value: `${formatTokens(item.total_tokens)} · ${Math.round(
                (item.total_tokens / total) * TELEMETRY_PERCENT_SCALE,
              )}%`,
            }))}
          />
          <AccessibleChartTable
            caption="Token usage by model"
            rows={chartData.map(({ provider, model, calls, total_tokens }) => ({
              provider,
              model,
              calls,
              total_tokens,
            }))}
            columns={[
              { key: "provider", label: "Provider" },
              { key: "model", label: "Model" },
              { key: "calls", label: "Calls" },
              { key: "total_tokens", label: "Total tokens" },
            ]}
          />
        </div>
      )}
    </ChartCard>
  )
}

function groupModelUsage(models: ModelUsageBreakdown[]): ModelChartItem[] {
  const visible = models.slice(0, MODEL_VISIBLE_LIMIT)
  if (models.length > MODEL_VISIBLE_LIMIT) {
    const overflow = models.slice(MODEL_VISIBLE_LIMIT - 1)
    visible.splice(MODEL_VISIBLE_LIMIT - 1, 1, {
      provider: "",
      model: "Other",
      calls: overflow.reduce((sum, item) => sum + item.calls, 0),
      tokens_in: overflow.reduce((sum, item) => sum + item.tokens_in, 0),
      tokens_out: overflow.reduce((sum, item) => sum + item.tokens_out, 0),
      total_tokens: overflow.reduce((sum, item) => sum + item.total_tokens, 0),
    })
  }

  return visible.map((item, index) => ({
    ...item,
    key: `${item.provider}:${item.model}`,
    color: TELEMETRY_CHART_SERIES[index % TELEMETRY_CHART_SERIES.length],
  }))
}
