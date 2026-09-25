import { formatTokens } from "@/features/members/components/usage-formatters"
import { formatEstimatedCost } from "@/features/resource-usage/components/resource-usage-formatters"
import type { MemberCostReportRow } from "@/shared/api/client"
import { PRIMARY_ROLE_LABELS, type PrimaryRole } from "@/shared/constants/member"
import { Badge } from "@/shared/ui/badge"
import { Table, TableBody, TableHead, TableRow, TableTd, TableTh, TableWrap } from "@/shared/ui/table"
import { NumCell, ShareBar } from "./model-cost-table"

const TH = "px-2.5 py-1.5"
const TD = "px-2.5 py-1.5"

const ROLE_TONE: Record<PrimaryRole, "accent" | "success" | "neutral"> = {
  admin: "accent",
  contribute: "success",
  user: "neutral",
}

export function MemberCostTable({
  rows,
  totals,
}: {
  rows: MemberCostReportRow[]
  totals?: { total_cost_usd_micros: number; total_tokens: number; cache_savings_usd_micros?: number }
}) {
  const grandCost = totals?.total_cost_usd_micros || 1

  return (
    <TableWrap className="rounded-none border-0">
      <Table className="text-xs">
        <TableHead>
          <tr>
            <TableTh className={`${TH} align-bottom`}>Member</TableTh>
            <TableTh className={`${TH} align-bottom text-right`}>Calls</TableTh>
            <TableTh className={`${TH} border-l border-(--border-soft) text-right font-normal`}>Input</TableTh>
            <TableTh className={`${TH} text-right font-normal`}>Output</TableTh>
            <TableTh className={`${TH} text-right font-normal`}>Cache RD</TableTh>
            <TableTh className={`${TH} text-right font-normal`}>Cache WR</TableTh>
            <TableTh className={`${TH} text-right`}>Total tokens</TableTh>
            <TableTh className={`${TH} border-l border-(--border-soft) text-right`}>Total cost</TableTh>
            <TableTh className={`${TH} text-right font-normal text-(--color-success)`}>Saved</TableTh>
            <TableTh className={`${TH} text-right`}>Avg $/1M</TableTh>
          </tr>
        </TableHead>
        <TableBody>
          {rows.map((row) => {
            const share = (row.total_cost_usd_micros / grandCost) * 100
            return (
              <TableRow key={row.user_id}>
                <TableTd className={TD}>
                  <div className="flex items-center gap-1.5">
                    <span className="grid size-5 shrink-0 place-items-center rounded-md bg-(--bg-key) text-[0.6rem] font-semibold text-(--color-text-muted)">
                      {row.display_name.charAt(0).toUpperCase()}
                    </span>
                    <div>
                      <div className="flex items-center gap-1.5">
                        <span className="font-medium">{row.display_name}</span>
                        <Badge tone={ROLE_TONE[row.primary_role]} className="text-[0.6rem] tracking-wide uppercase">
                          {PRIMARY_ROLE_LABELS[row.primary_role]}
                        </Badge>
                      </div>
                      <div className="text-[0.65rem] text-(--color-text-subtle)">{row.email}</div>
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
                <NumCell value={formatEstimatedCost(row.total_cost_usd_micros)} className={TD} border strong />
                <NumCell
                  value={row.cache_savings_usd_micros > 0 ? formatEstimatedCost(row.cache_savings_usd_micros) : "—"}
                  className={`${TD} text-(--color-success)`}
                  strong
                />
                <TableTd className={`${TD} text-right tabular-nums text-(--color-text-muted)`}>
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
              <TableTd colSpan={4} className={`${TD} border-l border-(--border-soft)`} />
              <TableTd className={`${TD} text-right`}>{formatTokens(totals.total_tokens)}</TableTd>
              <TableTd className={`${TD} border-l border-(--border-soft) text-right`}>{formatEstimatedCost(totals.total_cost_usd_micros)}</TableTd>
              <TableTd className={`${TD} text-right text-(--color-success)`}>
                {formatEstimatedCost(totals.cache_savings_usd_micros ?? 0)}
              </TableTd>
              <TableTd className={TD} />
            </tr>
          </tfoot>
        )}
      </Table>
    </TableWrap>
  )
}
