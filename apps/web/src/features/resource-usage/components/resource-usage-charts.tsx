import {
  Area,
  AreaChart,
  Bar,
  BarChart,
  CartesianGrid,
  Cell,
  Line,
  Pie,
  PieChart,
  XAxis,
  YAxis,
} from "recharts"

import { formatShortDate, formatTokens } from "@/features/members/components/usage-formatters"
import { formatEstimatedCost } from "@/features/resource-usage/components/resource-usage-formatters"
import type {
  ResourceUsageBreakdown,
  ResourceUsageDay,
  ResourceUsageMember,
  ResourceUsageModel,
  ResourceUsageRole,
  ResourceUsageTool,
} from "@/shared/api/client"
import { ChartCard } from "@/shared/components/chart-card"
import { ProviderBrandIcon } from "@/shared/components/provider-brand-icon"
import { TELEMETRY_CHART_SERIES } from "@/shared/constants/telemetry"
import {
  AccessibleChartTable,
  ChartContainer,
  ChartLegendList,
  ChartTooltip,
  ChartTooltipContent,
  type ChartConfig,
} from "@/shared/ui/chart"

const MARGIN = { top: 8, right: 8, bottom: 0, left: 0 } as const
const OUTCOME_CONFIG = {
  successes: { label: "Success", color: "var(--color-success)" },
  errors: { label: "Error", color: "var(--color-danger)" },
  blocked: { label: "Blocked", color: "var(--color-warning)" },
  cancelled: { label: "Cancelled", color: "var(--color-text-subtle)" },
} satisfies ChartConfig
const TOKEN_COST_CONFIG = {
  tokens_in: { label: "Input", color: "var(--chart-series-1)" },
  tokens_out: { label: "Output", color: "var(--chart-series-2)" },
  cache_read_tokens: { label: "Cache read", color: "var(--chart-series-3)" },
  reasoning_tokens: { label: "Reasoning", color: "var(--chart-series-4)" },
  tool_use_tokens: { label: "Tool use", color: "var(--chart-series-6)" },
  estimated_cost_usd_micros: { label: "Estimated cost", color: "var(--chart-series-5)" },
} satisfies ChartConfig
const RESOURCE_CONFIG = { uses: { label: "Uses", color: "var(--chart-series-1)" } } satisfies ChartConfig
const MODEL_CONFIG = { calls: { label: "Calls", color: "var(--chart-series-3)" } } satisfies ChartConfig
const MEMBER_CONFIG = { requests: { label: "Requests", color: "var(--chart-series-2)" } } satisfies ChartConfig
const ROLE_CONFIG = {
  model_calls: { label: "Model calls", color: "var(--chart-series-3)" },
  tool_calls: { label: "Tool calls", color: "var(--chart-series-6)" },
} satisfies ChartConfig
const TOOL_CONFIG = { calls: { label: "Calls", color: "var(--chart-series-6)" } } satisfies ChartConfig

export function RequestOutcomeChart({ daily }: { daily: ResourceUsageDay[] }) {
  return (
    <ChartCard title="Request outcomes" description="Terminal outcomes for requests that used governed resources.">
      <EmptyAware hasData={daily.length > 0}>
        <ChartContainer config={OUTCOME_CONFIG} className="h-64 w-full">
          <AreaChart accessibilityLayer data={daily} margin={MARGIN}>
            <CartesianGrid vertical={false} stroke="var(--border-soft)" strokeDasharray="3 3" />
            <XAxis dataKey="date" axisLine={false} tickLine={false} tickFormatter={formatShortDate} tick={{ fill: "var(--color-text-subtle)", fontSize: 10 }} />
            <YAxis allowDecimals={false} axisLine={false} tickLine={false} width={32} tick={{ fill: "var(--color-text-subtle)", fontSize: 10 }} />
            <ChartTooltip content={<ChartTooltipContent config={OUTCOME_CONFIG} />} />
            <Area type="monotone" dataKey="successes" stackId="outcome" fill="var(--color-success)" fillOpacity={0.24} stroke="var(--color-success)" />
            <Area type="monotone" dataKey="errors" stackId="outcome" fill="var(--color-danger)" fillOpacity={0.2} stroke="var(--color-danger)" />
            <Area type="monotone" dataKey="blocked" stackId="outcome" fill="var(--color-warning)" fillOpacity={0.18} stroke="var(--color-warning)" />
            <Area type="monotone" dataKey="cancelled" stackId="outcome" fill="var(--color-text-subtle)" fillOpacity={0.14} stroke="var(--color-text-subtle)" />
          </AreaChart>
        </ChartContainer>
        <Legend config={OUTCOME_CONFIG} />
        <AccessibleChartTable caption="Daily request outcomes" rows={daily.map((item) => ({ ...item }))} columns={[{ key: "date", label: "Date" }, { key: "successes", label: "Success" }, { key: "errors", label: "Error" }, { key: "blocked", label: "Blocked" }, { key: "cancelled", label: "Cancelled" }]} />
      </EmptyAware>
    </ChartCard>
  )
}

export function TokenCostChart({ daily }: { daily: ResourceUsageDay[] }) {
  return (
    <ChartCard title="Tokens & estimated cost" description="Input/output volume with priced model-call trend.">
      <EmptyAware hasData={daily.length > 0}>
        <ChartContainer config={TOKEN_COST_CONFIG} className="h-64 w-full">
          <AreaChart accessibilityLayer data={daily} margin={MARGIN}>
            <CartesianGrid vertical={false} stroke="var(--border-soft)" strokeDasharray="3 3" />
            <XAxis dataKey="date" axisLine={false} tickLine={false} tickFormatter={formatShortDate} tick={{ fill: "var(--color-text-subtle)", fontSize: 10 }} />
            <YAxis yAxisId="tokens" axisLine={false} tickLine={false} width={44} tickFormatter={formatTokens} tick={{ fill: "var(--color-text-subtle)", fontSize: 10 }} />
            <YAxis yAxisId="cost" orientation="right" axisLine={false} tickLine={false} width={54} tickFormatter={formatEstimatedCost} tick={{ fill: "var(--color-text-subtle)", fontSize: 10 }} />
            <ChartTooltip content={<ChartTooltipContent config={TOKEN_COST_CONFIG} />} />
            <Area yAxisId="tokens" type="monotone" dataKey="tokens_in" stackId="tokens" fill="var(--chart-series-1)" fillOpacity={0.22} stroke="var(--chart-series-1)" />
            <Area yAxisId="tokens" type="monotone" dataKey="tokens_out" stackId="tokens" fill="var(--chart-series-2)" fillOpacity={0.2} stroke="var(--chart-series-2)" />
            <Area yAxisId="tokens" type="monotone" dataKey="cache_read_tokens" stackId="tokens" fill="var(--chart-series-3)" fillOpacity={0.18} stroke="var(--chart-series-3)" />
            <Area yAxisId="tokens" type="monotone" dataKey="reasoning_tokens" stackId="tokens" fill="var(--chart-series-4)" fillOpacity={0.18} stroke="var(--chart-series-4)" />
            <Area yAxisId="tokens" type="monotone" dataKey="tool_use_tokens" stackId="tokens" fill="var(--chart-series-6)" fillOpacity={0.18} stroke="var(--chart-series-6)" />
            <Line yAxisId="cost" type="monotone" dataKey="estimated_cost_usd_micros" stroke="var(--chart-series-5)" strokeWidth={2} dot={false} />
          </AreaChart>
        </ChartContainer>
        <Legend config={TOKEN_COST_CONFIG} />
        <AccessibleChartTable caption="Daily token and cost usage" rows={daily.map((item) => ({ ...item }))} columns={[{ key: "date", label: "Date" }, { key: "tokens_in", label: "Input tokens" }, { key: "tokens_out", label: "Output tokens" }, { key: "cache_read_tokens", label: "Cache-read tokens" }, { key: "reasoning_tokens", label: "Reasoning tokens" }, { key: "tool_use_tokens", label: "Tool-use tokens" }, { key: "estimated_cost_usd_micros", label: "Cost in USD micros" }, { key: "unpriced_model_calls", label: "Unpriced calls" }]} />
      </EmptyAware>
    </ChartCard>
  )
}

export function ResourceShareChart({
  resources,
  description = "Agent, Skill and Plugin usage by version.",
}: {
  resources: ResourceUsageBreakdown[]
  description?: string
}) {
  const data = resources.slice(0, TELEMETRY_CHART_SERIES.length).map((item, index) => ({
    ...item,
    color: TELEMETRY_CHART_SERIES[index % TELEMETRY_CHART_SERIES.length],
  }))
  const total = data.reduce((sum, item) => sum + item.uses, 0)
  return (
    <ChartCard title="Resource share" description={description}>
      <EmptyAware hasData={data.length > 0}>
        <div className="grid gap-3 sm:grid-cols-[13rem_minmax(0,1fr)] sm:items-center">
          <div className="relative">
            <ChartContainer config={RESOURCE_CONFIG} className="h-52 w-full">
              <PieChart accessibilityLayer>
                <ChartTooltip content={<ChartTooltipContent config={RESOURCE_CONFIG} />} />
                <Pie data={data} dataKey="uses" nameKey="name" innerRadius={52} outerRadius={78} paddingAngle={2} stroke="var(--bg-card)" strokeWidth={2}>
                  {data.map((item) => <Cell key={`${item.resource_id}:${item.version_id}:${item.relation}`} fill={item.color} />)}
                </Pie>
              </PieChart>
            </ChartContainer>
            <div className="pointer-events-none absolute inset-0 grid place-items-center text-center"><div><div className="text-xl font-semibold tabular-nums">{total.toLocaleString()}</div><div className="text-[0.65rem] text-(--color-text-subtle)">resource uses</div></div></div>
          </div>
          <ChartLegendList items={data.map((item) => ({ key: `${item.resource_id}:${item.version_id}:${item.relation}`, label: `${item.name} · v${item.version}`, color: item.color, value: `${item.uses} uses` }))} />
        </div>
        <AccessibleChartTable caption="Resource usage share" rows={data.map((item) => ({ name: item.name, version: item.version, kind: item.kind, uses: item.uses, members: item.members }))} columns={[{ key: "name", label: "Resource" }, { key: "version", label: "Version" }, { key: "kind", label: "Kind" }, { key: "uses", label: "Uses" }, { key: "members", label: "Members" }]} />
      </EmptyAware>
    </ChartCard>
  )
}

export function ModelCallsChart({ models }: { models: ResourceUsageModel[] }) {
  const data = models.slice(0, 8).map((item) => ({ ...item, label: item.model }))
  return (
    <ChartCard title="Model calls" description="Calls and tokens generated while governed resources were active.">
      <EmptyAware hasData={data.length > 0}>
        <ChartContainer config={MODEL_CONFIG} className="h-52 w-full">
          <BarChart accessibilityLayer data={data} layout="vertical" margin={{ top: 4, right: 8, bottom: 0, left: 12 }}>
            <CartesianGrid horizontal={false} stroke="var(--border-soft)" strokeDasharray="3 3" />
            <XAxis type="number" allowDecimals={false} axisLine={false} tickLine={false} tick={{ fill: "var(--color-text-subtle)", fontSize: 10 }} />
            <YAxis type="category" dataKey="label" width={88} axisLine={false} tickLine={false} tick={{ fill: "var(--color-text-subtle)", fontSize: 10 }} />
            <ChartTooltip content={<ChartTooltipContent config={MODEL_CONFIG} />} />
            <Bar dataKey="calls" fill="var(--chart-series-3)" radius={[0, 4, 4, 0]} />
          </BarChart>
        </ChartContainer>
        <ChartLegendList className="mt-3" items={data.map((item) => ({ key: `${item.provider}:${item.model}`, label: item.model, color: "var(--chart-series-3)", icon: <ProviderBrandIcon providerId={item.provider} />, value: `${item.calls} calls · ${formatTokens(item.total_tokens)}` }))} />
        <AccessibleChartTable caption="Model calls" rows={data.map((item) => ({ provider: item.provider, model: item.model, calls: item.calls, total_tokens: item.total_tokens }))} columns={[{ key: "provider", label: "Provider" }, { key: "model", label: "Model" }, { key: "calls", label: "Calls" }, { key: "total_tokens", label: "Tokens" }]} />
      </EmptyAware>
    </ChartCard>
  )
}

export function MemberUsageChart({ members }: { members: ResourceUsageMember[] }) {
  const data = members.slice(0, 8).map((item) => ({ ...item, label: item.display_name }))
  return (
    <RankedBarChart
      title="Top members"
      description="Members with the most attributed requests in this range."
      data={data}
      dataKey="requests"
      config={MEMBER_CONFIG}
      tableCaption="Top members by attributed request count"
      tableColumns={[
        { key: "label", label: "Member" },
        { key: "primary_role", label: "Recorded role" },
        { key: "requests", label: "Requests" },
        { key: "resource_uses", label: "Resource uses" },
      ]}
    />
  )
}

export function RoleCallsChart({ roles }: { roles: ResourceUsageRole[] }) {
  const data = roles.map((item) => ({
    ...item,
    label: item.primary_role.charAt(0).toUpperCase() + item.primary_role.slice(1),
  }))
  return (
    <ChartCard title="Calls by recorded role" description="Model and tool calls grouped by the member role captured at ingest time.">
      <EmptyAware hasData={data.length > 0}>
        <ChartContainer config={ROLE_CONFIG} className="h-52 w-full">
          <BarChart accessibilityLayer data={data} layout="vertical" margin={{ top: 4, right: 8, bottom: 0, left: 12 }}>
            <CartesianGrid horizontal={false} stroke="var(--border-soft)" strokeDasharray="3 3" />
            <XAxis type="number" allowDecimals={false} axisLine={false} tickLine={false} tick={{ fill: "var(--color-text-subtle)", fontSize: 10 }} />
            <YAxis type="category" dataKey="label" width={76} axisLine={false} tickLine={false} tick={{ fill: "var(--color-text-subtle)", fontSize: 10 }} />
            <ChartTooltip content={<ChartTooltipContent config={ROLE_CONFIG} />} />
            <Bar dataKey="model_calls" stackId="calls" fill="var(--chart-series-3)" radius={[0, 0, 0, 0]} />
            <Bar dataKey="tool_calls" stackId="calls" fill="var(--chart-series-6)" radius={[0, 4, 4, 0]} />
          </BarChart>
        </ChartContainer>
        <Legend config={ROLE_CONFIG} />
        <AccessibleChartTable caption="Calls by recorded role" rows={data} columns={[{ key: "label", label: "Role" }, { key: "requests", label: "Requests" }, { key: "model_calls", label: "Model calls" }, { key: "tool_calls", label: "Tool calls" }, { key: "total_tokens", label: "Tokens" }]} />
      </EmptyAware>
    </ChartCard>
  )
}

export function ToolCallsChart({ tools }: { tools: ResourceUsageTool[] }) {
  const data = tools.slice(0, 8).map((item) => ({ ...item, label: item.tool_name }))
  return (
    <RankedBarChart
      title="Top tool calls"
      description="The tools called most often while these resources were active."
      data={data}
      dataKey="calls"
      config={TOOL_CONFIG}
      tableCaption="Top attributed tools"
      tableColumns={[
        { key: "label", label: "Tool" },
        { key: "category", label: "Category" },
        { key: "calls", label: "Calls" },
        { key: "errors", label: "Errors" },
      ]}
    />
  )
}

function RankedBarChart({
  title,
  description,
  data,
  dataKey,
  config,
  tableCaption,
  tableColumns,
}: {
  title: string
  description: string
  data: Array<Record<string, string | number>>
  dataKey: string
  config: ChartConfig
  tableCaption: string
  tableColumns: Array<{ key: string; label: string }>
}) {
  const color = Object.values(config)[0]?.color ?? "var(--chart-series-1)"
  return (
    <ChartCard title={title} description={description}>
      <EmptyAware hasData={data.length > 0}>
        <ChartContainer config={config} className="h-52 w-full">
          <BarChart accessibilityLayer data={data} layout="vertical" margin={{ top: 4, right: 8, bottom: 0, left: 12 }}>
            <CartesianGrid horizontal={false} stroke="var(--border-soft)" strokeDasharray="3 3" />
            <XAxis type="number" allowDecimals={false} axisLine={false} tickLine={false} tick={{ fill: "var(--color-text-subtle)", fontSize: 10 }} />
            <YAxis type="category" dataKey="label" width={96} axisLine={false} tickLine={false} tick={{ fill: "var(--color-text-subtle)", fontSize: 10 }} />
            <ChartTooltip content={<ChartTooltipContent config={config} />} />
            <Bar dataKey={dataKey} fill={color} radius={[0, 4, 4, 0]} />
          </BarChart>
        </ChartContainer>
        <AccessibleChartTable caption={tableCaption} rows={data} columns={tableColumns} />
      </EmptyAware>
    </ChartCard>
  )
}

function EmptyAware({ hasData, children }: { hasData: boolean; children: React.ReactNode }) {
  return hasData ? children : <div className="grid h-64 place-items-center text-sm text-(--color-text-muted)">No attributed usage in this range.</div>
}

function Legend({ config }: { config: ChartConfig }) {
  return <ChartLegendList className="mt-3 flex flex-wrap justify-center gap-x-5" items={Object.entries(config).map(([key, item]) => ({ key, label: item.label, color: item.color }))} />
}
