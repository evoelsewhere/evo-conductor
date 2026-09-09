import { USD_MICROS } from "@/shared/constants/resource-usage"

export function formatEstimatedCost(micros: number) {
  const dollars = micros / USD_MICROS
  return dollars < 0.01 && dollars > 0
    ? `$${dollars.toFixed(4)}`
    : dollars.toLocaleString(undefined, {
        style: "currency",
        currency: "USD",
        maximumFractionDigits: 2,
      })
}

/** Like `formatEstimatedCost`, but symmetric for negative numbers — a net
 * cache-savings figure can be negative (a call that wrote far more to cache
 * than it ever read back), and a tiny negative amount deserves the same
 * 4-decimal precision a tiny positive amount gets. */
export function formatCacheSavings(micros: number) {
  const dollars = micros / USD_MICROS
  return Math.abs(dollars) < 0.01 && dollars !== 0
    ? `${dollars < 0 ? "-" : ""}$${Math.abs(dollars).toFixed(4)}`
    : dollars.toLocaleString(undefined, {
        style: "currency",
        currency: "USD",
        maximumFractionDigits: 2,
      })
}

export function formatRelation(value: string) {
  return value
    .split("_")
    .map((part) => part[0]?.toUpperCase() + part.slice(1))
    .join(" ")
}
