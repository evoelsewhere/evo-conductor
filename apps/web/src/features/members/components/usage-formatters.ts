import {
  DURATION_INTEGER_SECONDS_THRESHOLD_MS,
  DURATION_SECOND_MS,
  TOKEN_COMPACT_INTEGER_THRESHOLD,
  TOKEN_MILLION,
  TOKEN_THOUSAND,
} from "@/shared/constants/telemetry"

export function formatTokens(value: number) {
  if (value >= TOKEN_MILLION) return `${(value / TOKEN_MILLION).toFixed(1)}M`
  if (value >= TOKEN_THOUSAND) {
    return `${(value / TOKEN_THOUSAND).toFixed(
      value >= TOKEN_COMPACT_INTEGER_THRESHOLD ? 0 : 1,
    )}K`
  }
  return formatNumber(value)
}

export function formatNumber(value: number) {
  return new Intl.NumberFormat().format(value)
}

export function formatDuration(value: number) {
  if (value < DURATION_SECOND_MS) return `${value} ms`
  return `${(value / DURATION_SECOND_MS).toFixed(
    value < DURATION_INTEGER_SECONDS_THRESHOLD_MS ? 1 : 0,
  )} s`
}

export function formatShortDate(value: string) {
  return new Date(`${value}T00:00:00`).toLocaleDateString(undefined, {
    month: "short",
    day: "numeric",
  })
}
