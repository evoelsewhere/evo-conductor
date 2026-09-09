import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"
import { RefreshCw, Tags } from "lucide-react"
import { useMemo, useState } from "react"

import {
  MODEL_PRICING_ALL_PROVIDERS,
  ModelPricingFilters,
} from "@/features/model-pricing/components/model-pricing-filters"
import { formatCatalogUpdatedAt } from "@/features/model-pricing/components/model-pricing-formatters"
import {
  ModelPricingTable,
  type ModelPricingSortDirection,
  type ModelPricingSortKey,
} from "@/features/model-pricing/components/model-pricing-table"
import { computeModelPricingRecommendations } from "@/features/model-pricing/lib/model-pricing-recommendations"
import { api, type ModelPricingCatalogEntry } from "@/shared/api/client"
import { PageFrame } from "@/shared/components/page-frame"
import { cn } from "@/shared/lib/utils"
import { Button } from "@/shared/ui/button"
import { EmptyState, ErrorState } from "@/shared/ui/empty-state"
import type { SearchableSelectOption } from "@/shared/ui/searchable-select"

const CATALOG_QUERY_KEY = ["model-pricing-catalog"] as const
const PAGE_SIZE = 25

function rate(entry: ModelPricingCatalogEntry, key: ModelPricingSortKey) {
  const value = key === "input" ? entry.pricing.base.input : entry.pricing.base.output
  // Unknown rates sort last, in either direction — there is nothing to
  // compare them against, so a null cost cannot claim to be "cheapest".
  return value ?? Number.POSITIVE_INFINITY
}

export function ModelPricingPage() {
  const queryClient = useQueryClient()
  const [search, setSearch] = useState("")
  const [provider, setProvider] = useState(MODEL_PRICING_ALL_PROVIDERS)
  const [sortKey, setSortKey] = useState<ModelPricingSortKey | null>(null)
  const [sortDirection, setSortDirection] = useState<ModelPricingSortDirection>("asc")
  const [page, setPage] = useState(0)

  const query = useQuery({
    queryKey: CATALOG_QUERY_KEY,
    queryFn: () => api.modelPricingCatalog(),
  })

  const refresh = useMutation({
    mutationFn: () => api.refreshModelPricingCatalog(),
    onSuccess: (snapshot) => {
      queryClient.setQueryData(CATALOG_QUERY_KEY, snapshot)
    },
  })

  const providerOptions = useMemo<SearchableSelectOption[]>(() => {
    const providers = new Set((query.data?.entries ?? []).map((entry) => entry.provider))
    return [
      { value: MODEL_PRICING_ALL_PROVIDERS, label: "All providers" },
      ...[...providers].sort().map((value) => ({ value, label: value })),
    ]
  }, [query.data])

  const visibleEntries = useMemo(() => {
    const entries = query.data?.entries ?? []
    const term = search.trim().toLowerCase()
    const filtered = entries.filter((entry) => {
      const matchesProvider =
        provider === MODEL_PRICING_ALL_PROVIDERS || entry.provider === provider
      const matchesSearch = term.length === 0 || entry.model.toLowerCase().includes(term)
      return matchesProvider && matchesSearch
    })
    if (!sortKey) return filtered
    const direction = sortDirection === "asc" ? 1 : -1
    return [...filtered].sort((a, b) => direction * (rate(a, sortKey) - rate(b, sortKey)))
  }, [query.data, search, provider, sortKey, sortDirection])

  const recommendations = useMemo(
    () => computeModelPricingRecommendations(visibleEntries),
    [visibleEntries],
  )

  const pageCount = Math.max(1, Math.ceil(visibleEntries.length / PAGE_SIZE))
  const currentPage = Math.min(page, pageCount - 1)
  const pageEntries = visibleEntries.slice(
    currentPage * PAGE_SIZE,
    currentPage * PAGE_SIZE + PAGE_SIZE,
  )

  function handleSearchChange(next: string) {
    setSearch(next)
    setPage(0)
  }

  function handleProviderChange(next: string) {
    setProvider(next)
    setPage(0)
  }

  function handleSort(key: ModelPricingSortKey) {
    if (sortKey !== key) {
      setSortKey(key)
      setSortDirection("asc")
    } else {
      setSortDirection((current) => (current === "asc" ? "desc" : "asc"))
    }
    setPage(0)
  }

  return (
    <PageFrame
      title="Model Pricing"
      subtitle="The world AI pricing catalog Conductor knows about — a reference price list, not your organization's spend (see Usage for that)."
      action={
        <div className="flex items-center gap-2">
          <Button
            variant="outline"
            size="sm"
            disabled={refresh.isPending}
            onClick={() => refresh.mutate()}
          >
            <RefreshCw className={cn("size-3.5", refresh.isPending && "animate-spin")} />
            {refresh.isPending ? "Refreshing…" : "Refresh catalog"}
          </Button>
          <span
            aria-live="polite"
            className="text-[0.68rem] whitespace-nowrap text-(--color-text-subtle)"
          >
            {refresh.isPending
              ? "Fetching…"
              : `Updated ${formatCatalogUpdatedAt(query.data?.fetched_at ?? null)}`}
          </span>
        </div>
      }
    >
      <div className="flex flex-col gap-4">
        <ModelPricingFilters
          search={search}
          onSearchChange={handleSearchChange}
          provider={provider}
          providerOptions={providerOptions}
          onProviderChange={handleProviderChange}
        />

        {query.isLoading ? (
          <EmptyState icon={Tags} title="Loading the pricing catalog…" />
        ) : query.isError ? (
          <ErrorState message="Could not load the pricing catalog. Try again in a moment." />
        ) : visibleEntries.length === 0 ? (
          <EmptyState
            icon={Tags}
            title="No models match these filters"
            description="Clear the search or provider filter, or refresh the catalog if it has never been fetched."
          />
        ) : (
          <ModelPricingTable
            entries={pageEntries}
            sortKey={sortKey}
            sortDirection={sortDirection}
            onSort={handleSort}
            recommendations={recommendations}
            footer={
              pageCount > 1 ? (
                <>
                  <span>
                    {currentPage * PAGE_SIZE + 1}–
                    {Math.min(currentPage * PAGE_SIZE + PAGE_SIZE, visibleEntries.length)} of{" "}
                    {visibleEntries.length}
                  </span>
                  <div className="flex gap-2">
                    <Button
                      size="sm"
                      variant="outline"
                      disabled={currentPage === 0}
                      onClick={() => setPage(currentPage - 1)}
                    >
                      Previous
                    </Button>
                    <Button
                      size="sm"
                      variant="outline"
                      disabled={currentPage >= pageCount - 1}
                      onClick={() => setPage(currentPage + 1)}
                    >
                      Next
                    </Button>
                  </div>
                </>
              ) : undefined
            }
          />
        )}
      </div>
    </PageFrame>
  )
}
