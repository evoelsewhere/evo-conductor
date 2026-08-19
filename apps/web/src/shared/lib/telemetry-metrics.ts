/**
 * Percentage of successful governed requests across every terminal outcome.
 *
 * `requests` is the authoritative denominator and includes success, error,
 * blocked, and cancelled requests. A range without terminal requests has no
 * rate, so callers can preserve their screen-specific empty-state copy.
 */
export function terminalRequestSuccessRate(
  successes: number | null | undefined,
  requests: number | null | undefined,
) {
  if (!requests) return null
  return Math.round(((successes ?? 0) / requests) * 100)
}

/**
 * Provider total usage is input plus output. Cache-read, reasoning and tool-use
 * values are diagnostic subsets of those totals and must not be added again.
 */
export function inputOutputTokenTotal(
  tokensIn: number | null | undefined,
  tokensOut: number | null | undefined,
) {
  return (tokensIn ?? 0) + (tokensOut ?? 0)
}
