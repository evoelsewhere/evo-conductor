import type { DashboardAttentionItem } from "@/features/dashboard/lib/dashboard-model"

export function formatDashboardUpdatedTime(timestamp: number) {
  return new Intl.DateTimeFormat(undefined, {
    hour: "2-digit",
    minute: "2-digit",
  }).format(timestamp)
}

export function dashboardInitials(value: string) {
  return value
    .trim()
    .split(/\s+/)
    .slice(0, 2)
    .map((part) => part[0]?.toUpperCase())
    .join("")
}

export function dashboardAttentionTone(tone: DashboardAttentionItem["tone"]) {
  if (tone === "danger") return "danger"
  if (tone === "warning") return "warning"
  return "accent"
}

export function formatOptionalCount(value: number | null | undefined) {
  return value == null ? "Not reported" : value.toLocaleString()
}

export function formatOptionalPercent(value: number | null | undefined) {
  const normalized = normalizePercent(value)
  return normalized == null ? "Not reported" : `${normalized.toFixed(1)}%`
}

export function normalizePercent(value: number | null | undefined) {
  if (value == null || !Number.isFinite(value)) return null
  return Math.min(100, Math.max(0, Math.round(value * 10) / 10))
}

export function percentOf(
  used: number | null | undefined,
  total: number | null | undefined,
) {
  if (used == null || total == null || total <= 0) return null
  return normalizePercent((used / total) * 100)
}

export function formatBytePair(
  used: number | null | undefined,
  total: number | null | undefined,
) {
  if (used == null || total == null) return "Not reported"
  return `${formatBytes(used)} / ${formatBytes(total)}`
}

function formatBytes(value: number) {
  if (!Number.isFinite(value) || value < 0) return "Not reported"
  const units = ["B", "KB", "MB", "GB", "TB"]
  let amount = value
  let unitIndex = 0
  while (amount >= 1024 && unitIndex < units.length - 1) {
    amount /= 1024
    unitIndex += 1
  }
  return `${new Intl.NumberFormat(undefined, {
    maximumFractionDigits: amount >= 10 || unitIndex === 0 ? 0 : 1,
  }).format(amount)} ${units[unitIndex]}`
}

export function formatThreshold(seconds: number) {
  if (!Number.isFinite(seconds) || seconds <= 0) {
    return "an unreported window"
  }
  if (seconds < 60) return `${Math.round(seconds)} sec`
  if (seconds < 3_600) return `${Math.round(seconds / 60)} min`
  return `${Math.round(seconds / 3_600)} hr`
}

export function formatTimestamp(value: string | undefined) {
  if (!value) return "at an unreported time"
  const timestamp = new Date(value)
  if (Number.isNaN(timestamp.getTime())) return "at an unreported time"
  return `at ${new Intl.DateTimeFormat(undefined, {
    hour: "2-digit",
    minute: "2-digit",
  }).format(timestamp)}`
}
