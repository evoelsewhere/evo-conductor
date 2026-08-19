import {
  ChartSkeletons,
  TelemetryReadiness,
} from "@/features/dashboard/components/dashboard-states"
import { hasDashboardTelemetry } from "@/features/dashboard/lib/dashboard-model"
import {
  RequestOutcomeChart,
  TokenCostChart,
} from "@/features/resource-usage/components/resource-usage-charts"
import type {
  DashboardSummary,
  ResourceUsageAnalytics,
  ResourceUsageScope,
} from "@/shared/api/client"
import { Card, CardContent } from "@/shared/ui/card"
import { ErrorState } from "@/shared/ui/empty-state"

export function DashboardAnalyticsPanel({
  analytics,
  summary,
  isLoading,
  error,
  analyticsHref,
  scope,
  announceLoading = true,
}: {
  analytics: ResourceUsageAnalytics | undefined
  summary: DashboardSummary | undefined
  isLoading: boolean
  error: Error | null
  analyticsHref: string
  scope: ResourceUsageScope
  announceLoading?: boolean
}) {
  return (
    <div className="min-w-0 xl:col-span-8">
      {isLoading ? (
        <ChartSkeletons announce={announceLoading} />
      ) : analytics && hasDashboardTelemetry(analytics) ? (
        <div className="grid min-w-0 gap-4 md:grid-cols-2">
          <RequestOutcomeChart daily={analytics.daily} scope={scope} />
          <TokenCostChart daily={analytics.daily} scope={scope} />
        </div>
      ) : error ? (
        <Card>
          <CardContent>
            <ErrorState message={`${scope === "all" ? "Project" : "Governed"} analytics are unavailable. Live project state remains visible.`} />
          </CardContent>
        </Card>
      ) : (
        <TelemetryReadiness
          hasConnections={
            (summary?.presence?.members_seen_recently ??
              summary?.members_online ??
              0) > 0
          }
          allRequests={analytics?.totals.all_requests ?? 0}
          analyticsHref={analyticsHref}
          scope={scope}
        />
      )}
    </div>
  )
}
