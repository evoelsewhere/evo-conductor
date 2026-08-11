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

export function formatRelation(value: string) {
  return value
    .split("_")
    .map((part) => part[0]?.toUpperCase() + part.slice(1))
    .join(" ")
}
