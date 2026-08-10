/**
 * Provider brand icons shared with EvoFlux.
 *
 * The SVG glyphs are vendored from EvoFlux's local LobeHub-derived icon set
 * (MIT) and rendered as CSS masks so Conductor keeps its dependency graph
 * small while retaining crisp, theme-safe brand colors.
 */
import {
  PROVIDER_ALIASES,
  PROVIDER_BRANDS,
  PROVIDER_FALLBACK_COLOR,
  PROVIDER_FALLBACK_ID,
  PROVIDER_FALLBACK_LABEL,
  PROVIDER_GLYPH_SIZE_CLASSES,
  PROVIDER_ICON_SIZE_CLASSES,
  type ProviderIconSize,
} from "@/shared/constants/provider"
import { cn } from "@/shared/lib/utils"

function providerPrefix(modelOrProviderId: string) {
  const [prefix] = modelOrProviderId.trim().split(":", 1)
  const normalized = prefix.toLowerCase().replace(/[\s-]+/g, "_")
  return PROVIDER_ALIASES[normalized] ?? normalized
}

export function ProviderBrandIcon({
  providerId,
  size = "xs",
  className,
}: {
  providerId?: string | null
  size?: ProviderIconSize
  className?: string
}) {
  const id = providerPrefix(providerId || PROVIDER_FALLBACK_ID)
  const brand = PROVIDER_BRANDS[id] ?? {
    color: PROVIDER_FALLBACK_COLOR,
    label: providerId || PROVIDER_FALLBACK_LABEL,
  }
  return (
    <span
      className={cn(
        "grid shrink-0 place-items-center transition-colors duration-150",
        PROVIDER_ICON_SIZE_CLASSES[size],
        className,
      )}
      style={{
        backgroundColor: `color-mix(in srgb, ${brand.color} 14%, transparent)`,
        boxShadow: `inset 0 0 0 1px color-mix(in srgb, ${brand.color} 24%, transparent)`,
      }}
      title={brand.label}
      aria-hidden="true"
    >
      {brand.imageUrl ? (
        <img
          src={brand.imageUrl}
          alt=""
          className={cn("object-contain", PROVIDER_GLYPH_SIZE_CLASSES[size])}
        />
      ) : brand.maskUrl ? (
        <span
          className={PROVIDER_GLYPH_SIZE_CLASSES[size]}
          style={{
            backgroundColor: brand.color,
            maskImage: `url("${brand.maskUrl}")`,
            maskPosition: "center",
            maskRepeat: "no-repeat",
            maskSize: "contain",
            WebkitMaskImage: `url("${brand.maskUrl}")`,
            WebkitMaskPosition: "center",
            WebkitMaskRepeat: "no-repeat",
            WebkitMaskSize: "contain",
          }}
        />
      ) : (
        <span className="text-[0.65rem] leading-none font-bold" style={{ color: brand.color }}>
          {id.charAt(0).toUpperCase()}
        </span>
      )}
    </span>
  )
}
