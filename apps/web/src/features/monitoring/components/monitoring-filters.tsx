import { RotateCcw, Search } from "lucide-react"

import type { Tag } from "@/shared/api/client"
import {
  MONITORING_ALL_FILTER,
  MONITORING_ROLE_OPTIONS,
} from "@/shared/constants/monitoring"
import { Button } from "@/shared/ui/button"
import { Input } from "@/shared/ui/input"
import { Select } from "@/shared/ui/select"

export interface MonitoringFilterState {
  primaryRole: string
  tagId: string
  provider: string
  model: string
}

export const EMPTY_MONITORING_FILTERS: MonitoringFilterState = {
  primaryRole: MONITORING_ALL_FILTER,
  tagId: MONITORING_ALL_FILTER,
  provider: "",
  model: "",
}

export function MonitoringFilters({
  value,
  tags,
  tagsLoading,
  onChange,
}: {
  value: MonitoringFilterState
  tags: Tag[]
  tagsLoading?: boolean
  onChange: (value: MonitoringFilterState) => void
}) {
  const set = (key: keyof MonitoringFilterState, next: string) =>
    onChange({ ...value, [key]: next })
  const activeCount = Object.entries(value).filter(([key, item]) =>
    key === "provider" || key === "model" ? Boolean(item) : item !== MONITORING_ALL_FILTER,
  ).length

  return (
    <div className="flex flex-wrap items-center gap-2 rounded-xl border border-(--border-card) bg-(--bg-card) p-3">
      <Select
        value={value.primaryRole}
        onValueChange={(next) => set("primaryRole", next)}
        options={[...MONITORING_ROLE_OPTIONS]}
        aria-label="Filter by role"
        className="lg:w-40"
      />
      <Select
        value={value.tagId}
        onValueChange={(next) => set("tagId", next)}
        options={[
          {
            value: MONITORING_ALL_FILTER,
            label: tagsLoading ? "Loading tags…" : "All tags",
          },
          ...tags.map((tag) => ({ value: tag.id, label: tag.name })),
        ]}
        disabled={tagsLoading}
        aria-busy={tagsLoading}
        aria-label="Filter by tag"
        className="lg:w-40"
      />
      <SearchField value={value.provider} onChange={(next) => set("provider", next)} placeholder="Provider" ariaLabel="Filter by provider" />
      <SearchField value={value.model} onChange={(next) => set("model", next)} placeholder="Model" ariaLabel="Filter by model" />
      {activeCount > 0 && (
        <Button variant="ghost" size="sm" onClick={() => onChange(EMPTY_MONITORING_FILTERS)}>
          <RotateCcw className="size-3.5" /> Clear filters
        </Button>
      )}
    </div>
  )
}

function SearchField({
  value,
  onChange,
  placeholder,
  ariaLabel,
}: {
  value: string
  onChange: (value: string) => void
  placeholder: string
  ariaLabel: string
}) {
  return (
    <label className="relative">
      <Search className="pointer-events-none absolute top-1/2 left-3 size-3.5 -translate-y-1/2 text-(--color-text-subtle)" />
      <Input className="w-36 pl-8" value={value} onChange={(event) => onChange(event.target.value)} placeholder={placeholder} aria-label={ariaLabel} />
    </label>
  )
}
