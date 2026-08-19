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
import { LiveOperations } from "@/features/dashboard/components/live-operations"
import { MemberActivityPanel } from "@/features/dashboard/components/member-activity-panel"
import { RoleAndWorkspace } from "@/features/dashboard/components/role-and-workspace"
import { TopSignals } from "@/features/dashboard/components/top-signals"
import { useDashboardData } from "@/features/dashboard/hooks/use-dashboard-data"
import { dashboardResourceTotal } from "@/features/dashboard/lib/dashboard-model"
import { PageFrame } from "@/shared/components/page-frame"
import { useMinimumLoading } from "@/shared/hooks/use-minimum-loading"
import { useUiStore } from "@/shared/stores/ui"

export function DashboardPage() {
  const setSettingsOpen = useUiStore((state) => state.setSettingsOpen)
  const {
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
  } = useDashboardData()
  const summaryLoading = useMinimumLoading(summary.isLoading && !summary.data)
  const analyticsLoading = useMinimumLoading(analytics.isLoading && !analytics.data)

  return (
    <PageFrame
      title="Dashboard"
      subtitle={
        summary.data
          ? `${summary.data.project_name} · Current project health, received EvoFlux requests and governed-resource coverage.`
          : "Current project health, received EvoFlux requests and governed-resource coverage."
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
        <div
          className="grid gap-4"
          aria-busy={
            isRefreshing && !summaryLoading && !analyticsLoading
              ? true
              : undefined
          }
        >
          <span className="sr-only" role="status" aria-live="polite" aria-atomic="true">
            {summaryLoading || analyticsLoading ? "Loading dashboard sections…" : ""}
          </span>
          {((summary.error && !summaryLoading) ||
            (analytics.error && !analyticsLoading) ||
            pending.error) && (
            <PartialErrorPanel
              summaryError={summaryLoading ? null : summary.error}
              analyticsError={analyticsLoading ? null : analytics.error}
              pendingError={canManageMembers ? pending.error : null}
              onRetry={refreshDashboard}
            />
          )}

          {!analyticsLoading && attention.length > 0 && (
            <DashboardAttentionRail
              items={attention}
              overviewHref={overviewHref}
            />
          )}

          <DashboardMetricGrid
            summary={summary.data}
            analytics={analytics.data}
            summaryLoading={summaryLoading}
            analyticsLoading={analyticsLoading}
          />

          <div className="grid items-start gap-4 xl:grid-cols-12">
            <DashboardAnalyticsPanel
              analytics={analytics.data}
              summary={summary.data}
              isLoading={analyticsLoading}
              error={analytics.error}
              analyticsHref={overviewHref()}
              announceLoading={false}
            />
            <LiveOperations
              className="xl:col-span-4"
              summary={summary.data}
              analytics={analytics.data}
              loading={summaryLoading}
              announceLoading={false}
            />
          </div>

          {canReadMemberTelemetry && (
            <MemberActivityPanel
              members={analytics.data?.members ?? []}
              loading={analyticsLoading}
              analyticsHref={analyticsHref}
              announceLoading={false}
            />
          )}

          <div className="grid items-start gap-4 xl:grid-cols-12">
            <TopSignals
              className="xl:col-span-8"
              resources={analytics.data?.resources ?? []}
              models={analytics.data?.models ?? []}
              tools={analytics.data?.tools ?? []}
              loading={analyticsLoading}
              analyticsHref={analyticsHref}
              announceLoading={false}
            />
            <RoleAndWorkspace
              className="xl:col-span-4"
              roles={analytics.data?.roles ?? []}
              summary={summary.data}
              loading={analyticsLoading}
              summaryLoading={summaryLoading}
              analyticsHref={analyticsHref}
              canReadMembers={canReadMembers}
              canReadTaxonomy={canReadTaxonomy}
              canReadSettings={canReadSettings}
              onOpenSettings={() => setSettingsOpen(true)}
              announceLoading={false}
            />
          </div>

          {!summaryLoading &&
            summary.data &&
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
