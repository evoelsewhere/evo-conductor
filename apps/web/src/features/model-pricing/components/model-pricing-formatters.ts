export function formatRatePerMillion(rate: number | null) {
  if (rate === null) return "—"
  if (rate === 0) return "Free"
  return rate.toLocaleString(undefined, {
    style: "currency",
    currency: "USD",
    minimumFractionDigits: 2,
    maximumFractionDigits: 4,
  })
}

export function formatCatalogUpdatedAt(fetchedAt: string | null) {
  if (!fetchedAt) return "Never fetched yet"
  return new Date(fetchedAt).toLocaleString()
}
