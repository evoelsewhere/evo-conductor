import { Code2, PanelsTopLeft, Waypoints } from "lucide-react"

import {
  RESOURCE_TARGET_MODE,
  type ResourceTargetMode,
} from "@/shared/constants/resource"
import { cn } from "@/shared/lib/utils"

const MODE_OPTIONS = [
  {
    value: RESOURCE_TARGET_MODE.WORK,
    label: "Work",
    description: "Cowork, research, browser and document sessions",
    icon: PanelsTopLeft,
  },
  {
    value: RESOURCE_TARGET_MODE.CODING,
    label: "Coding",
    description: "Repository, project, worktree and review sessions",
    icon: Code2,
  },
  {
    value: RESOURCE_TARGET_MODE.AIM,
    label: "AIM",
    description: "Governed modernization, traceability and migration pipelines",
    icon: Waypoints,
  },
] as const

export function ResourceModeSelector({
  value,
  onChange,
  disabled,
}: {
  value: readonly ResourceTargetMode[]
  onChange: (value: ResourceTargetMode[]) => void
  disabled?: boolean
}) {
  function toggle(mode: ResourceTargetMode) {
    if (disabled) return
    const selected = value.includes(mode)
    if (selected && value.length === 1) return
    onChange(
      MODE_OPTIONS.map((option) => option.value).filter((candidate) =>
        candidate === mode ? !selected : value.includes(candidate),
      ),
    )
  }

  return (
    <div className="grid gap-2 sm:grid-cols-3" aria-label="EvoFlux availability modes">
      {MODE_OPTIONS.map((option) => {
        const Icon = option.icon
        const selected = value.includes(option.value)
        return (
          <button
            key={option.value}
            type="button"
            aria-pressed={selected}
            disabled={disabled}
            onClick={() => toggle(option.value)}
            className={cn(
              "flex min-h-20 items-start gap-3 rounded-lg border px-3 py-3 text-left outline-none transition-colors focus-visible:ring-2 focus-visible:ring-(--focus-ring)/40 disabled:cursor-not-allowed disabled:opacity-60",
              selected
                ? "border-(--color-accent) bg-(--color-accent-soft)/45"
                : "border-(--border-soft) bg-(--bg-page)/45 hover:border-(--color-border-strong)",
            )}
          >
            <span
              className={cn(
                "mt-0.5 grid size-8 shrink-0 place-items-center rounded-lg",
                selected
                  ? "bg-(--color-accent)/12 text-(--color-accent)"
                  : "bg-(--bg-key) text-(--color-text-subtle)",
              )}
            >
              <Icon className="size-4" aria-hidden="true" />
            </span>
            <span className="min-w-0">
              <span className="block text-sm font-medium text-(--color-text)">
                {option.label}
              </span>
              <span className="mt-0.5 block text-[0.7rem] leading-relaxed text-(--color-text-muted)">
                {option.description}
              </span>
            </span>
          </button>
        )
      })}
    </div>
  )
}
