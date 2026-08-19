import { useQuery } from "@tanstack/react-query"
import { useMemo, useState } from "react"

import {
  DASHBOARD_TOP_SIGNAL_LIMIT,
  type DashboardRangeDays,
} from "@/features/dashboard/lib/dashboard-config"
import {
  buildDashboardAttention,
  DASHBOARD_QUERY_KEYS,
  dashboardAnalyticsHref,
  dashboardUpdatedAt,
} from "@/features/dashboard/lib/dashboard-model"
import { api } from "@/shared/api/client"
import { MILLISECONDS_PER_DAY, UsageRangePreset } from "@/shared/constants/telemetry"
import { PERMISSION, mayRequest } from "@/shared/lib/authorization"
import { useMinimumLoading } from "@/shared/hooks/use-minimum-loading"
import { useAuthStore } from "@/shared/stores/auth"

export function useDashboardData() {
  const authorization = useAuthStore((state) => state.authorization)
  const can = useAuthStore((state) => state.can)
  const [rangeDays, setRangeDays] = useState<DashboardRangeDays>(30)
  const [rangeAnchor, setRangeAnchor] = useState(() => Date.now())
  const dateRange = useMemo(
    () => ({
      from: new Date(
        rangeAnchor - rangeDays * MILLISECONDS_PER_DAY,
      ).toISOString(),
      to: new Date(rangeAnchor).toISOString(),
    }),
    [rangeAnchor, rangeDays],
  )
  const hrefPreset =
    rangeDays === 1
      ? UsageRangePreset.Day
      : rangeDays === 7
        ? UsageRangePreset.Week
        : rangeDays === 30
          ? UsageRangePreset.Month
          : UsageRangePreset.Custom
  const currentRole = authorization?.current_role
  const canManageMembers = mayRequest(can(PERMISSION.MEMBER_MANAGE))
  const canReadMembers = mayRequest(can(PERMISSION.MEMBER_DIRECTORY_READ))
  const canReadMemberTelemetry = mayRequest(
    can(PERMISSION.TELEMETRY_MEMBER_READ_ANY),
  )
  const canReadTaxonomy = mayRequest(can(PERMISSION.TAXONOMY_READ))
  const canReadSettings = mayRequest(can(PERMISSION.PROJECT_SETTINGS_READ))

  const summary = useQuery({
    queryKey: DASHBOARD_QUERY_KEYS.summary,
    queryFn: api.dashboard,
    staleTime: 20_000,
    refetchInterval: 30_000,
    refetchIntervalInBackground: false,
  })
  const analytics = useQuery({
    queryKey: DASHBOARD_QUERY_KEYS.analytics(
      currentRole,
      dateRange.from,
      dateRange.to,
    ),
    queryFn: () =>
      api.resourceUsage({
        ...dateRange,
        limit: DASHBOARD_TOP_SIGNAL_LIMIT,
      }),
    staleTime: 60_000,
    placeholderData: (previousData, previousQuery) =>
      previousQuery?.queryKey[2] === currentRole ? previousData : undefined,
  })
  const pending = useQuery({
    queryKey: ["pending-count"],
    queryFn: api.pendingCount,
    enabled: canManageMembers,
    staleTime: 30_000,
  })

  const attention = useMemo(
    () =>
      buildDashboardAttention(
        analytics.data,
        canManageMembers ? (pending.data?.count ?? 0) : 0,
      ),
    [analytics.data, canManageMembers, pending.data?.count],
  )
  const analyticsHref = (filters: Record<string, string> = {}) =>
    dashboardAnalyticsHref(
      "/app/resources/usage/usage",
      hrefPreset,
      dateRange.from.slice(0, 10),
      dateRange.to.slice(0, 10),
      filters,
    )
  const overviewHref = (filters: Record<string, string> = {}) =>
    dashboardAnalyticsHref(
      "/app/resources/usage",
      hrefPreset,
      dateRange.from.slice(0, 10),
      dateRange.to.slice(0, 10),
      filters,
    )
  const updatedAt = dashboardUpdatedAt(
    summary.dataUpdatedAt,
    analytics.dataUpdatedAt,
    canManageMembers ? pending.dataUpdatedAt : 0,
  )
  const isRefreshing =
    summary.isFetching || analytics.isFetching || pending.isFetching
  const isInitialLoading = useMinimumLoading(
    !summary.data &&
      !analytics.data &&
      (summary.isLoading || analytics.isLoading),
  )

  function refreshDashboard() {
    setRangeAnchor(Date.now())
    void Promise.all([
      summary.refetch(),
      ...(canManageMembers ? [pending.refetch()] : []),
    ])
  }

  function changeRange(next: DashboardRangeDays) {
    setRangeDays(next)
    setRangeAnchor(Date.now())
  }

  return {
    analytics,
    analyticsHref,
    attention,
    canManageMembers,
    canReadMembers,
    canReadMemberTelemetry,
    canReadSettings,
    canReadTaxonomy,
    changeRange,
    isInitialLoading,
    isRefreshing,
    overviewHref,
    pending,
    rangeDays,
    refreshDashboard,
    summary,
    updatedAt,
  }
}
