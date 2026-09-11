import { formatTokens } from "@/features/members/components/usage-formatters"
import { formatEstimatedCost } from "@/features/resource-usage/components/resource-usage-formatters"
import type { ModelCostReportRow } from "@/shared/api/client"
import { ProviderBrandIcon } from "@/shared/components/provider-brand-icon"
import { Badge } from "@/shared/ui/badge"
import { LoadingState, Skeleton } from "@/shared/ui/skeleton"
import {
  Table,
  TableBody,
  TableHead,
  TableRow,
  TableTd,
  TableTh,
  TableWrap,
} from "@/shared/ui/table"

const TH = "px-2.5 py-1.5"
const TD = "px-2.5 py-1.5"

export function ModelCostTable({
  rows,
  totals,
}: {
  rows: ModelCostReportRow[]
  totals?: { total_cost_usd_micros: number; total_tokens: number }
}) {
  const grandCost = totals?.total_cost_usd_micros || 1

  return (
    <TableWrap className="rounded-none border-0">
      <Table className="text-xs">
        <TableHead>
          <tr>
            <TableTh rowSpan={2} className={`${TH} align-bottom`}>Model</TableTh>
            <TableTh rowSpan={2} className={`${TH} align-bottom text-right`}>Calls</TableTh>
            <TableTh colSpan={5} className={`${TH} border-l border-(--border-soft) text-center`}>Tokens burned</TableTh>
            <TableTh colSpan={5} className={`${TH} border-l border-(--border-soft) text-center`}>Cost — USD</TableTh>
            <TableTh rowSpan={2} className={`${TH} border-l border-(--border-soft) align-bottom text-right`}>Avg $/1M</TableTh>
          </tr>
          <tr>
            <TableTh className={`${TH} border-l border-(--border-soft) text-right font-normal`}>Input</TableTh>
            <TableTh className={`${TH} text-right font-normal`}>Output</TableTh>
            <TableTh className={`${TH} text-right font-normal`}>Cache RD</TableTh>
            <TableTh className={`${TH} text-right font-normal`}>Cache WR</TableTh>
            <TableTh className={`${TH} text-right`}>Total</TableTh>
            <TableTh className={`${TH} border-l border-(--border-soft) text-right font-normal`}>Input</TableTh>
            <TableTh className={`${TH} text-right font-normal`}>Output</TableTh>
            <TableTh className={`${TH} text-right font-normal`}>Cache RD</TableTh>
            <TableTh className={`${TH} text-right font-normal`}>Cache WR</TableTh>
            <TableTh className={`${TH} text-right`}>Total</TableTh>
          </tr>
        </TableHead>
        <TableBody>
          {rows.map((row) => {
            const share = (row.total_cost_usd_micros / grandCost) * 100
            return (
              <TableRow key={`${row.provider}:${row.model}`}>
                <TableTd className={TD}>
                  <div className="flex items-center gap-1.5">
                    <ProviderBrandIcon providerId={row.provider} size="xs" />
                    <div>
                      <div className="flex items-center gap-1.5">
                        <span className="font-medium">{row.model}</span>
                        <Badge tone="neutral" className="text-[0.6rem] tracking-wide uppercase">{row.provider}</Badge>
                      </div>
                      <ShareBar
                        segments={[
                          { tokens: row.input_tokens, color: "var(--color-accent)" },
                          { tokens: row.output_tokens, color: "var(--color-success)" },
                          { tokens: row.cache_read_tokens + row.cache_write_tokens, color: "var(--color-warning)" },
                        ]}
                        totalTokens={row.total_tokens}
                        sharePercent={share}
                      />
                    </div>
                  </div>
                </TableTd>
                <TableTd className={`${TD} text-right tabular-nums`}>
                  {row.calls.toLocaleString()}
                  {row.unpriced_calls > 0 && (
                    <div className="mt-1"><Badge tone="warning">{row.unpriced_calls} unpriced</Badge></div>
                  )}
                </TableTd>
                <NumCell value={formatTokens(row.input_tokens)} className={TD} border />
                <NumCell value={formatTokens(row.output_tokens)} className={TD} />
                <NumCell value={formatTokens(row.cache_read_tokens)} className={TD} />
                <NumCell value={formatTokens(row.cache_write_tokens)} className={TD} />
                <NumCell value={formatTokens(row.total_tokens)} className={TD} strong />
                <NumCell value={formatEstimatedCost(row.input_cost_usd_micros)} className={TD} border />
                <NumCell value={formatEstimatedCost(row.output_cost_usd_micros)} className={TD} />
                <NumCell value={formatEstimatedCost(row.cache_read_cost_usd_micros)} className={TD} />
                <NumCell value={formatEstimatedCost(row.cache_write_cost_usd_micros)} className={TD} />
                <NumCell value={formatEstimatedCost(row.total_cost_usd_micros)} className={TD} strong />
                <TableTd className={`${TD} border-l border-(--border-soft) text-right tabular-nums text-(--color-text-muted)`}>
                  {formatEstimatedCost(row.avg_usd_micros_per_million_tokens)}
                </TableTd>
              </TableRow>
            )
          })}
        </TableBody>
        {totals && (
          <tfoot>
            <tr className="border-t border-(--border-soft) font-medium">
              <TableTd colSpan={2} className={TD}>Total</TableTd>
              <TableTd colSpan={5} className={`${TD} border-l border-(--border-soft) text-right`}>{formatTokens(totals.total_tokens)}</TableTd>
              <TableTd colSpan={5} className={`${TD} border-l border-(--border-soft) text-right`}>{formatEstimatedCost(totals.total_cost_usd_micros)}</TableTd>
              <TableTd className={`${TD} border-l border-(--border-soft)`} />
            </tr>
          </tfoot>
        )}
      </Table>
    </TableWrap>
  )
}

/** Composition of a row's own tokens, plus its share of the whole report's spend. */
export function ShareBar({
  segments,
  totalTokens,
  sharePercent,
}: {
  segments: { tokens: number; color: string }[]
  totalTokens: number
  sharePercent: number
}) {
  const total = totalTokens || 1
  return (
    <div className="mt-1 flex items-center gap-1.5">
      <div className="flex h-1 w-14 overflow-hidden rounded-full bg-(--bg-key)">
        {segments.map((segment, index) => (
          <div key={index} style={{ width: `${(segment.tokens / total) * 100}%`, background: segment.color }} />
        ))}
      </div>
      <span className="text-[0.65rem] tabular-nums text-(--color-text-subtle)">
        {sharePercent.toFixed(1)}%
      </span>
    </div>
  )
}

export function NumCell({
  value,
  border,
  strong,
  className,
}: {
  value: string
  border?: boolean
  strong?: boolean
  className?: string
}) {
  return (
    <TableTd
      className={`${className ?? ""} text-right tabular-nums ${border ? "border-l border-(--border-soft) " : ""}${strong ? "font-semibold" : "text-(--color-text-muted)"}`}
    >
      {value}
    </TableTd>
  )
}

export function CostTableSkeleton({ columns, label }: { columns: number; label: string }) {
  const gridStyle = { gridTemplateColumns: `repeat(${columns}, minmax(0, 1fr))` }
  return (
    <LoadingState label={label} className="overflow-x-auto">
      <div className="min-w-[64rem] overflow-hidden">
        <div className="grid gap-4 border-b border-(--border-soft) bg-(--bg-key)/30 px-4 py-3" style={gridStyle}>
          {Array.from({ length: columns }, (_, index) => (
            <Skeleton key={index} className="h-3 w-16 max-w-full" />
          ))}
        </div>
        <div className="divide-y divide-(--border-soft)">
          {Array.from({ length: 5 }, (_, row) => (
            <div key={row} className="grid items-center gap-4 px-4 py-3.5" style={gridStyle}>
              {Array.from({ length: columns }, (_, column) => (
                <Skeleton key={column} className={column === 0 ? "h-4 w-full" : "h-3.5 w-3/4"} />
              ))}
            </div>
          ))}
        </div>
      </div>
    </LoadingState>
  )
}
