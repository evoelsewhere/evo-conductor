import { BrandLogo } from "@/shared/components/logo"
import { cn } from "@/shared/lib/utils"

const titleSizes = {
  sm: "text-[0.8rem]",
  md: "text-sm",
  lg: "text-lg",
} as const

export function BrandMark({
  className,
  size = "md",
  /** Hides the wordmark so only the tile shows — used by the collapsed sidebar. */
  compact = false,
  tagline = "master control for EvoFlux",
}: {
  className?: string
  size?: "sm" | "md" | "lg"
  compact?: boolean
  tagline?: string | null
}) {
  return (
    <div className={cn("flex min-w-0 items-center gap-2.5", className)}>
      <BrandLogo size={size} />
      {!compact && (
        <div className="min-w-0 leading-tight">
          <div
            className={cn(
              "truncate font-semibold tracking-tight text-(--color-text)",
              titleSizes[size],
            )}
          >
            Evo Conductor
          </div>
          {tagline && (
            <div className="truncate text-[0.7rem] text-(--color-text-subtle)">
              {tagline}
            </div>
          )}
        </div>
      )}
    </div>
  )
}
