import { BrandLogo } from "@/shared/components/logo"
import { cn } from "@/shared/lib/utils"

const titleSizes = {
  sm: "text-[0.8rem]",
  md: "text-sm",
  lg: "text-lg",
} as const

const logoTileSizes = {
  sm: "size-7 rounded-md",
  md: "size-8 rounded-lg",
  lg: "size-11 rounded-xl",
} as const

const DEFAULT_TITLE = "Evo Conductor"
const DEFAULT_TAGLINE = "master control for EvoFlux"

export function BrandMark({
  className,
  size = "md",
  /** Hides the wordmark so only the tile shows — used by the collapsed sidebar. */
  compact = false,
  /** Project display name / project name. Falls back to Evo Conductor. */
  title,
  /** Secondary line under the title. Pass `null` to hide. */
  tagline,
  /** Custom project logo URL. Falls back to the EvoFlux mark. */
  logoUrl,
}: {
  className?: string
  size?: "sm" | "md" | "lg"
  compact?: boolean
  title?: string | null
  tagline?: string | null
  logoUrl?: string | null
}) {
  const resolvedTitle = title?.trim() || DEFAULT_TITLE
  const resolvedTagline =
    tagline === undefined
      ? title?.trim()
        ? null
        : DEFAULT_TAGLINE
      : tagline
  const customLogo = logoUrl?.trim() || null

  return (
    <div className={cn("flex min-w-0 items-center gap-2.5", className)}>
      {customLogo ? (
        <span
          className={cn(
            "grid shrink-0 place-items-center overflow-hidden bg-(--bg-key) shadow-[0_2px_10px_-3px_rgba(0,0,0,0.35)]",
            logoTileSizes[size],
          )}
        >
          <img
            src={customLogo}
            alt=""
            className="size-full object-cover"
          />
        </span>
      ) : (
        <BrandLogo size={size} />
      )}
      {!compact && (
        <div className="min-w-0 leading-tight">
          <div
            className={cn(
              "truncate font-semibold tracking-tight text-(--color-text)",
              titleSizes[size],
            )}
          >
            {resolvedTitle}
          </div>
          {resolvedTagline && (
            <div className="truncate text-[0.7rem] text-(--color-text-subtle)">
              {resolvedTagline}
            </div>
          )}
        </div>
      )}
    </div>
  )
}
