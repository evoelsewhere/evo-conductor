import type { ModelPricingCatalogEntry } from "@/shared/api/client"

export interface ModelPricingRecommendations {
  cheapestInputKey: string | null
  cheapestOutputKey: string | null
  bestValueKey: string | null
}

export function modelPricingEntryKey(entry: ModelPricingCatalogEntry) {
  return `${entry.provider}:${entry.model}`
}

/**
 * Cheapest-input, cheapest-output and best-value picks within the entries
 * passed in — recomputed against whatever the current filters leave
 * visible, not the whole catalog, so a badge always points at something
 * still on screen.
 *
 * "Best value" weighs input at 3x output: a typical turn reads far more
 * tokens than it writes, so a model billed evenly per direction is not
 * actually the cheaper one to run.
 */
export function computeModelPricingRecommendations(
  entries: readonly ModelPricingCatalogEntry[],
): ModelPricingRecommendations {
  let cheapestInput: ModelPricingCatalogEntry | null = null
  let cheapestOutput: ModelPricingCatalogEntry | null = null
  let bestValue: ModelPricingCatalogEntry | null = null
  let bestValueScore = Number.POSITIVE_INFINITY

  for (const entry of entries) {
    const input = entry.pricing.base.input
    const output = entry.pricing.base.output

    if (input !== null && (cheapestInput === null || input < cheapestInput.pricing.base.input!)) {
      cheapestInput = entry
    }
    if (
      output !== null &&
      (cheapestOutput === null || output < cheapestOutput.pricing.base.output!)
    ) {
      cheapestOutput = entry
    }
    if (input !== null && output !== null) {
      const score = input * 3 + output
      if (score < bestValueScore) {
        bestValueScore = score
        bestValue = entry
      }
    }
  }

  return {
    cheapestInputKey: cheapestInput ? modelPricingEntryKey(cheapestInput) : null,
    cheapestOutputKey: cheapestOutput ? modelPricingEntryKey(cheapestOutput) : null,
    bestValueKey: bestValue ? modelPricingEntryKey(bestValue) : null,
  }
}
