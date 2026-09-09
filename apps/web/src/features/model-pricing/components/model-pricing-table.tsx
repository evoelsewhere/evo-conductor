import { ArrowDown, ArrowUp, ArrowUpDown } from "lucide-react"

import { formatRatePerMillion } from "@/features/model-pricing/components/model-pricing-formatters"
import {
  modelPricingEntryKey,
  type ModelPricingRecommendations,
} from "@/features/model-pricing/lib/model-pricing-recommendations"
import type { ModelPricingCatalogEntry } from "@/shared/api/client"
import { ProviderBrandIcon } from "@/shared/components/provider-brand-icon"
import { cn } from "@/shared/lib/utils"
import { Badge } from "@/shared/ui/badge"
import {
  Table,
  TableBody,
  TableHead,
  TableRow,
  TableTd,
  TableTh,
  TableWrap,
} from "@/shared/ui/table"

export type ModelPricingSortKey = "input" | "output"
export type ModelPricingSortDirection = "asc" | "desc"

function tierTitle(entry: ModelPricingCatalogEntry) {
  return entry.pricing.tiers
    .map((tier) => {
      const input = tier.rates.input ?? entry.pricing.base.input
      const output = tier.rates.output ?? entry.pricing.base.output
      return `Above ${tier.above_tokens.toLocaleString()} tokens: input ${formatRatePerMillion(input)}, output ${formatRatePerMillion(output)}`
    })
    .join("\n")
}

function SortableHeader({
  label,
  sortKey,
  activeSortKey,
  direction,
  onSort,
}: {
  label: string
  sortKey: ModelPricingSortKey
  activeSortKey: ModelPricingSortKey | null
  direction: ModelPricingSortDirection
  onSort: (key: ModelPricingSortKey) => void
}) {
  const active = activeSortKey === sortKey
  const Icon = active ? (direction === "asc" ? ArrowUp : ArrowDown) : ArrowUpDown
  return (
    <button
      type="button"
      onClick={() => onSort(sortKey)}
      className={cn(
        "inline-flex items-center gap-1 hover:text-(--color-text)",
        active && "text-(--color-text)",
      )}
    >
      {label}
      <Icon className="size-3" />
    </button>
  )
}

export function ModelPricingTable({
  entries,
  sortKey,
  sortDirection,
  onSort,
  footer,
  recommendations,
}: {
  entries: ModelPricingCatalogEntry[]
  sortKey: ModelPricingSortKey | null
  sortDirection: ModelPricingSortDirection
  onSort: (key: ModelPricingSortKey) => void
  footer?: React.ReactNode
  recommendations?: ModelPricingRecommendations
}) {
  return (
    <TableWrap>
      <Table>
        <TableHead>
          <tr>
            <TableTh>Provider / model</TableTh>
            <TableTh>
              <SortableHeader
                label="Input / 1M"
                sortKey="input"
                activeSortKey={sortKey}
                direction={sortDirection}
                onSort={onSort}
              />
            </TableTh>
            <TableTh>
              <SortableHeader
                label="Output / 1M"
                sortKey="output"
                activeSortKey={sortKey}
                direction={sortDirection}
                onSort={onSort}
              />
            </TableTh>
            <TableTh>Cache read / 1M</TableTh>
            <TableTh>Cache write / 1M</TableTh>
            <TableTh>Reasoning / 1M</TableTh>
            <TableTh>Long-context tiers</TableTh>
          </tr>
        </TableHead>
        <TableBody>
          {entries.map((entry) => {
            const key = modelPricingEntryKey(entry)
            const isCheapestInput = recommendations?.cheapestInputKey === key
            const isCheapestOutput = recommendations?.cheapestOutputKey === key
            const isBestValue = recommendations?.bestValueKey === key
            return (
              <TableRow key={key}>
                <TableTd>
                  <div className="flex items-center gap-2">
                    <ProviderBrandIcon providerId={entry.provider} />
                    <div>
                      <div className="flex items-center gap-1.5">
                        <span className="font-medium">{entry.model}</span>
                        {isBestValue && (
                          <Badge
                            tone="warning"
                            title="Lowest cost for a typical turn (input weighted 3x output)"
                          >
                            Best value
                          </Badge>
                        )}
                      </div>
                      <div className="text-xs text-(--color-text-subtle)">
                        {entry.provider}
                      </div>
                    </div>
                  </div>
                </TableTd>
                <TableTd className="tabular-nums">
                  <div className="flex items-center gap-1.5">
                    {formatRatePerMillion(entry.pricing.base.input)}
                    {isCheapestInput && <Badge tone="success">Cheapest</Badge>}
                  </div>
                </TableTd>
                <TableTd className="tabular-nums">
                  <div className="flex items-center gap-1.5">
                    {formatRatePerMillion(entry.pricing.base.output)}
                    {isCheapestOutput && <Badge tone="success">Cheapest</Badge>}
                  </div>
                </TableTd>
                <TableTd className="tabular-nums">
                  {formatRatePerMillion(entry.pricing.base.cache_read)}
                </TableTd>
                <TableTd className="tabular-nums">
                  {formatRatePerMillion(entry.pricing.base.cache_write)}
                </TableTd>
                <TableTd className="tabular-nums">
                  {formatRatePerMillion(entry.pricing.base.reasoning)}
                </TableTd>
                <TableTd>
                  {entry.pricing.tiers.length > 0 ? (
                    <Badge tone="accent" title={tierTitle(entry)}>
                      +{entry.pricing.tiers.length} tier
                      {entry.pricing.tiers.length > 1 ? "s" : ""}
                    </Badge>
                  ) : (
                    <span className="text-(--color-text-subtle)">—</span>
                  )}
                </TableTd>
              </TableRow>
            )
          })}
        </TableBody>
      </Table>
      {footer && (
        <div className="flex items-center justify-between border-t border-(--border-soft) px-4 py-3 text-xs text-(--color-text-muted)">
          {footer}
        </div>
      )}
    </TableWrap>
  )
}
