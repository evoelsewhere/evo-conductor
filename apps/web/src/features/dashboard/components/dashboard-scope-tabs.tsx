import { RadioTower, ShieldCheck } from "lucide-react"

import type {
  ResourceUsageScope,
  ResourceUsageTotals,
} from "@/shared/api/client"
import { cn } from "@/shared/lib/utils"

const SCOPE_OPTIONS = [
  {
    value: "all",
    label: "All EvoFlux activity",
    description: "Every project event accepted by Conductor.",
    icon: RadioTower,
  },
  {
    value: "governed",
    label: "Governed activity",
    description: "Only activity linked to a managed resource.",
    icon: ShieldCheck,
  },
] as const satisfies ReadonlyArray<{
  value: ResourceUsageScope
  label: string
  description: string
  icon: typeof RadioTower
}>

export function DashboardScopeTabs({
  value,
  totals,
  loading,
  onChange,
}: {
  value: ResourceUsageScope
  totals: ResourceUsageTotals | undefined
  loading: boolean
  onChange: (scope: ResourceUsageScope) => void
}) {
  return (
    <section
      className="rounded-xl border border-(--border-soft) bg-(--bg-card) p-1.5"
      aria-labelledby="dashboard-scope-heading"
    >
      <h2 id="dashboard-scope-heading" className="sr-only">
        Dashboard activity scope
      </h2>
      <div
        className="grid gap-1 sm:grid-cols-2"
        role="group"
        aria-label="Dashboard activity scope"
      >
        {SCOPE_OPTIONS.map((option) => {
          const selected = value === option.value
          const count =
            option.value === "all"
              ? totals?.all_requests
              : totals?.governed_requests
          const Icon = option.icon
          return (
            <button
              key={option.value}
              type="button"
              aria-pressed={selected}
              onClick={() => onChange(option.value)}
              className={cn(
                "flex min-w-0 items-center gap-3 rounded-lg border px-3 py-2.5 text-left outline-none transition-colors focus-visible:ring-2 focus-visible:ring-(--focus-ring)/35",
                selected
                  ? "border-(--color-accent)/35 bg-(--color-accent-soft)"
                  : "border-transparent hover:border-(--border-soft) hover:bg-(--bg-key)/45",
              )}
            >
              <span
                className={cn(
                  "grid size-8 shrink-0 place-items-center rounded-md",
                  selected
                    ? "bg-(--color-accent) text-white"
                    : "bg-(--bg-key) text-(--color-text-muted)",
                )}
              >
                <Icon className="size-4" />
              </span>
              <span className="min-w-0 flex-1">
                <span className="flex items-center justify-between gap-2">
                  <span className="truncate text-sm font-semibold">
                    {option.label}
                  </span>
                  <span className="shrink-0 text-xs font-semibold tabular-nums text-(--color-text-muted)">
                    {loading || count == null ? "—" : count.toLocaleString()}
                    <span className="ml-1 font-normal">requests</span>
                  </span>
                </span>
                <span className="mt-0.5 block text-xs text-(--color-text-subtle)">
                  {option.description}
                </span>
              </span>
            </button>
          )
        })}
      </div>
      <p className="px-2 pb-1 pt-2 text-[0.7rem] leading-5 text-(--color-text-subtle)">
        Requests, outcomes, tokens, cost, members, models and tools all follow
        this scope. Live connections, host metrics and delivery state remain
        current project snapshots.
      </p>
    </section>
  )
}
