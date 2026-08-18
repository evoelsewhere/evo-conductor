import { DashboardAnalyticsPanel } from "@/features/dashboard/components/dashboard-analytics-panel"
import {
  DashboardAttentionRail,
  DashboardMetricGrid,
} from "@/features/dashboard/components/dashboard-overview"
import {
  DashboardSkeleton,
  GettingStarted,
  PartialErrorPanel,
} from "@/features/dashboard/components/dashboard-states"
import { DashboardToolbar } from "@/features/dashboard/components/dashboard-toolbar"
import { useDashboardData } from "@/features/dashboard/hooks/use-dashboard-data"
import { dashboardResourceTotal } from "@/features/dashboard/lib/dashboard-model"
import { PageFrame } from "@/shared/components/page-frame"

export function DashboardPage() {
  const {
    analytics,
    attention,
    canManageMembers,
    changeRange,
    isInitialLoading,
    isRefreshing,
    overviewHref,
    pending,
    rangeDays,
    refreshDashboard,
    summary,
    updatedAt,
  } = useDashboardData()

  return (
    <PageFrame
      title="Dashboard"
      subtitle={
        summary.data
          ? `${summary.data.project_name} · Current project health and selected-range governed activity.`
          : "Current project health and selected-range governed activity."
      }
      className="max-w-[100rem]"
      action={
        <DashboardToolbar
          rangeDays={rangeDays}
          isRefreshing={isRefreshing}
          updatedAt={updatedAt}
          onRangeChange={changeRange}
          onRefresh={refreshDashboard}
        />
      }
    >
      {isInitialLoading ? (
        <DashboardSkeleton />
      ) : (
        <div className="grid gap-4">
          {(summary.error || analytics.error || pending.error) && (
            <PartialErrorPanel
              summaryError={summary.error}
              analyticsError={analytics.error}
              pendingError={canManageMembers ? pending.error : null}
              onRetry={refreshDashboard}
            />
          )}

          {attention.length > 0 && (
            <DashboardAttentionRail
              items={attention}
              overviewHref={overviewHref}
            />
          )}

          <DashboardMetricGrid
            summary={summary.data}
            analytics={analytics.data}
          />

          <DashboardAnalyticsPanel
            analytics={analytics.data}
            summary={summary.data}
            isLoading={analytics.isLoading}
            error={analytics.error}
            analyticsHref={overviewHref()}
          />

          {summary.data &&
            summary.data.members_total <= 1 &&
            summary.data.secrets_active === 0 &&
            dashboardResourceTotal(summary.data) === 0 && (
              <GettingStarted canManageMembers={canManageMembers} />
            )}
        </div>
      )}
    </PageFrame>
  )
}
