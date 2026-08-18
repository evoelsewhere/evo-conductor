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
