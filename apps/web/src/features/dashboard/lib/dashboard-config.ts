import type { PrimaryRole } from "@/shared/api/client"
import { PRIMARY_ROLE } from "@/shared/constants/member"

export const DASHBOARD_TOP_SIGNAL_LIMIT = 3

export const DASHBOARD_RANGE_OPTIONS = [7, 30, 90] as const

export type DashboardRangeDays = (typeof DASHBOARD_RANGE_OPTIONS)[number]

export const DASHBOARD_ROLE_COLORS: Record<PrimaryRole, string> = {
  [PRIMARY_ROLE.ADMIN]: "var(--chart-series-2)",
  [PRIMARY_ROLE.CONTRIBUTE]: "var(--chart-series-1)",
  [PRIMARY_ROLE.USER]: "var(--chart-series-4)",
}
