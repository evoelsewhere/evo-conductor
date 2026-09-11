const MICROS_PER_USD = 1_000_000

/**
 * Parse a plain dollar amount into micro-USD, or return null.
 *
 * Reads the digits rather than going through a float, and refuses anything
 * finer than a micro-dollar instead of rounding it away: an allowance stored
 * as a slightly different number than the one typed would only surface when
 * the limit fired at the wrong point. Mirrors the `limits set` command.
 */
export function parseUsdToMicros(input: string): number | null {
  const text = input.trim()
  const [whole = "", fraction = ""] = text.split(".", 2)
  if (text.split(".").length > 2) return null
  if (whole === "" && fraction === "") return null
  if (!/^\d*$/.test(whole) || !/^\d*$/.test(fraction)) return null
  if (fraction.length > 6) return null

  const dollars = whole === "" ? 0 : Number(whole)
  const micros = Number(fraction.padEnd(6, "0"))
  const total = dollars * MICROS_PER_USD + micros
  return Number.isSafeInteger(total) ? total : null
}

/**
 * Render micro-USD exactly: two decimals where that is the whole of it, six
 * where it is not, so an amount is never shown as one nobody could have typed.
 */
export function formatMicrosAsUsd(micros: number): string {
  const dollars = Math.floor(micros / MICROS_PER_USD)
  const fraction = micros % MICROS_PER_USD
  return fraction % 10_000 === 0
    ? `${dollars}.${String(fraction / 10_000).padStart(2, "0")}`
    : `${dollars}.${String(fraction).padStart(6, "0")}`
}
