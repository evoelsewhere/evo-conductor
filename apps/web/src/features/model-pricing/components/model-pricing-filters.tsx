import { Search } from "lucide-react"

import { Input } from "@/shared/ui/input"
import {
  SearchableSelect,
  type SearchableSelectOption,
} from "@/shared/ui/searchable-select"

export const MODEL_PRICING_ALL_PROVIDERS = "__all__"

export function ModelPricingFilters({
  search,
  onSearchChange,
  provider,
  providerOptions,
  onProviderChange,
}: {
  search: string
  onSearchChange: (value: string) => void
  provider: string
  providerOptions: readonly SearchableSelectOption[]
  onProviderChange: (value: string) => void
}) {
  return (
    <div className="flex flex-col gap-2 sm:flex-row sm:items-center">
      <div className="relative flex-1">
        <Search className="pointer-events-none absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2 text-(--color-text-subtle)" />
        <Input
          className="pl-8"
          value={search}
          onChange={(event) => onSearchChange(event.target.value)}
          placeholder="Search by model name…"
          aria-label="Search by model name"
        />
      </div>
      <SearchableSelect
        value={provider}
        onValueChange={onProviderChange}
        options={providerOptions}
        placeholder="Search providers…"
        aria-label="Filter by provider"
        className="sm:w-56"
      />
    </div>
  )
}
