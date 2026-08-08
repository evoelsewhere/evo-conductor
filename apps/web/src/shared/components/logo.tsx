import { useId } from "react"

import { cn } from "@/shared/lib/utils"

/**
 * EvoFlux mark — rounded diamond with the flow glyph knocked out.
 * Vector source: `src/assets/brand/evoflux-logo.svg` (copied from the evoflux repo).
 *
 * The glyph is masked rather than filled so the cut-out stays transparent and
 * picks up whatever tile or background sits behind it.
 */
export function EvoFluxGlyph({ className }: { className?: string }) {
  const maskId = useId()

  return (
    <svg
      viewBox="0 0 500 500"
      aria-hidden="true"
      className={cn("size-4", className)}
    >
      <mask id={maskId}>
        <g transform="rotate(45 250 250)">
          <rect
            x="110"
            y="110"
            width="280"
            height="280"
            rx="65"
            ry="65"
            fill="#fff"
            stroke="#fff"
            strokeWidth="20"
          />
          <circle cx="250" cy="250" r="18" fill="#000" />
          <path
            d="M 160,200 L 310,200 A 25,25 0 0,1 335,225 L 335,230"
            fill="none"
            stroke="#000"
            strokeWidth="38"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
          <path
            d="M 340,300 L 190,300 A 25,25 0 0,1 165,275 L 165,270"
            fill="none"
            stroke="#000"
            strokeWidth="38"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        </g>
      </mask>
      <rect
        width="500"
        height="500"
        fill="currentColor"
        mask={`url(#${maskId})`}
      />
    </svg>
  )
}

const tileSizes = {
  sm: "size-7 rounded-md",
  md: "size-8 rounded-lg",
  lg: "size-11 rounded-xl",
} as const

const glyphSizes = {
  sm: "size-6",
  md: "size-7",
  lg: "size-9.5",
} as const

/**
 * The mark on the EvoFlux ink tile — the standalone app icon.
 * The tile stays dark in both themes because it is the brand surface.
 */
export function BrandLogo({
  size = "md",
  className,
}: {
  size?: keyof typeof tileSizes
  className?: string
}) {
  return (
    <span
      className={cn(
        "grid shrink-0 place-items-center bg-[#111111] text-white shadow-[0_2px_10px_-3px_rgba(0,0,0,0.55)]",
        tileSizes[size],
        className,
      )}
    >
      <EvoFluxGlyph className={glyphSizes[size]} />
    </span>
  )
}
