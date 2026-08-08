import { Monitor, Moon, Sun } from "lucide-react"

import { useThemeStore, type ThemeMode } from "@/shared/stores/theme"
import { Tooltip } from "@/shared/ui/tooltip"
import { cn } from "@/shared/lib/utils"

const icons: Record<ThemeMode, typeof Sun> = {
  light: Sun,
  dark: Moon,
  system: Monitor,
}

const labels: Record<ThemeMode, string> = {
  light: "Light theme",
  dark: "Dark theme",
  system: "Match system theme",
}

/** Cycling icon button — compact enough for the collapsed sidebar. */
export function ThemeToggle({
  className,
  showLabel = false,
}: {
  className?: string
  showLabel?: boolean
}) {
  const mode = useThemeStore((s) => s.mode)
  const cycle = useThemeStore((s) => s.cycle)
  const Icon = icons[mode]

  return (
    <Tooltip content={labels[mode]} side="top" disabled={showLabel}>
      <button
        type="button"
        onClick={cycle}
        aria-label={labels[mode]}
        className={cn(
          "inline-flex items-center gap-2 rounded-md text-(--color-text-muted) transition-colors outline-none hover:bg-(--bg-key) hover:text-(--color-text) focus-visible:ring-2 focus-visible:ring-(--focus-ring)/40",
          showLabel ? "w-full px-2.5 py-2 text-sm" : "size-8 justify-center",
          className,
        )}
      >
        <Icon className="size-4 shrink-0" strokeWidth={1.7} />
        {showLabel && <span className="capitalize">{mode}</span>}
      </button>
    </Tooltip>
  )
}
