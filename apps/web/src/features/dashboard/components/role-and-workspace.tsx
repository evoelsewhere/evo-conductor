import {
  Boxes,
  Gauge,
  KeyRound,
  Radio,
  Shield,
  Star,
  Tags,
  Users,
  type LucideIcon,
} from "lucide-react"

import { DASHBOARD_ROLE_COLORS } from "@/features/dashboard/lib/dashboard-config"
import { dashboardResourceTotal } from "@/features/dashboard/lib/dashboard-model"
import type {
  DashboardSummary,
  ResourceUsageRole,
} from "@/shared/api/client"
import { PRIMARY_ROLE_LABELS } from "@/shared/constants/member"
import { Badge } from "@/shared/ui/badge"
import { Button, buttonVariants } from "@/shared/ui/button"
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/shared/ui/card"
import { LoadingState, Skeleton } from "@/shared/ui/skeleton"

export function RoleAndWorkspace({
  roles,
  summary,
  loading,
  summaryLoading = false,
  analyticsHref,
  canReadMembers,
  canReadTaxonomy,
  canReadSettings,
  onOpenSettings,
  className,
  announceLoading = true,
}: {
  roles: ResourceUsageRole[]
  summary: DashboardSummary | undefined
  loading: boolean
  summaryLoading?: boolean
  analyticsHref: (filters?: Record<string, string>) => string
  canReadMembers: boolean
  canReadTaxonomy: boolean
  canReadSettings: boolean
  onOpenSettings: () => void
  className?: string
  announceLoading?: boolean
}) {
  const roleTotal = roles.reduce((sum, item) => sum + item.requests, 0)
  const resourceTotal = dashboardResourceTotal(summary)

  return (
    <Card className={className}>
      <CardHeader>
        <div>
          <CardTitle>Project context</CardTitle>
          <CardDescription className="mt-0.5">
            Recorded role usage, feedback, project facts and destinations.
          </CardDescription>
        </div>
      </CardHeader>
      <CardContent className="grid gap-4">
        <section aria-labelledby="dashboard-role-usage">
          <div className="flex items-center justify-between gap-2">
            <h3 id="dashboard-role-usage" className="text-xs font-semibold">
              Role usage
            </h3>
            <Badge>Recorded at ingestion</Badge>
          </div>
          {loading ? (
            <LoadingState label="Loading role usage" announce={announceLoading} className="mt-3 grid gap-2">
              <Skeleton className="h-2 w-full" />
              <Skeleton className="h-6 w-full" />
              <Skeleton className="h-6 w-full" />
            </LoadingState>
          ) : roleTotal > 0 ? (
            <>
              <div
                role="img"
                aria-label={roles
                  .map(
                    (item) =>
                      `${PRIMARY_ROLE_LABELS[item.primary_role]} ${item.requests} requests`,
                  )
                  .join(", ")}
                className="mt-3 flex h-2 overflow-hidden rounded-full bg-(--bg-key)"
              >
                {roles.map((item) => (
                  <span
                    key={item.primary_role}
                    style={{
                      width: `${(item.requests / roleTotal) * 100}%`,
                      backgroundColor: DASHBOARD_ROLE_COLORS[item.primary_role],
                    }}
                  />
                ))}
              </div>
              <div className="mt-2 grid gap-1">
                {roles.map((item) => (
                  <a
                    key={item.primary_role}
                    href={analyticsHref({ primary_role: item.primary_role })}
                    className="flex items-center gap-2 rounded-md px-1 py-1 text-xs outline-none hover:bg-(--bg-key) focus-visible:ring-2 focus-visible:ring-(--focus-ring)/35"
                  >
                    <span
                      className="size-2 rounded-full"
                      style={{
                        backgroundColor:
                          DASHBOARD_ROLE_COLORS[item.primary_role],
                      }}
                    />
                    <span className="min-w-0 flex-1 text-(--color-text-muted)">
                      {PRIMARY_ROLE_LABELS[item.primary_role]}
                    </span>
                    <span className="font-medium tabular-nums">
                      {item.requests.toLocaleString()}
                    </span>
                  </a>
                ))}
              </div>
            </>
          ) : (
            <p className="mt-2 text-xs text-(--color-text-subtle)">
              Role usage appears after governed telemetry arrives.
            </p>
          )}
        </section>

        <FeedbackPanel
          feedback={summary?.feedback}
          loading={summaryLoading}
          announceLoading={false}
        />

        <div className="border-t border-(--border-soft) pt-4">
          <h3 className="text-xs font-semibold">Workspace pulse</h3>
          {summaryLoading ? (
            <LoadingState label="Loading workspace pulse" announce={false} className="mt-2 grid gap-2">
              {Array.from({ length: 3 }, (_, index) => (
                <div key={index} className="flex items-center gap-2">
                  <Skeleton className="size-6" />
                  <Skeleton className="h-3 flex-1" />
                  <Skeleton className="h-3 w-16" />
                </div>
              ))}
            </LoadingState>
          ) : (
            <dl className="mt-2 grid gap-2 text-xs">
              <WorkspaceDatum
                label="Published catalog · 4 kinds"
                value={summary ? resourceTotal.toLocaleString() : "—"}
                icon={Boxes}
              />
              <WorkspaceDatum
                label="Unrevoked connection tokens"
                value={summary ? summary.secrets_active.toLocaleString() : "—"}
                icon={KeyRound}
              />
              <WorkspaceDatum
                label="Authentication"
                value={
                  summary
                    ? summary.sso_enabled
                      ? "SSO enabled"
                      : "Password only"
                    : "—"
                }
                icon={Radio}
              />
            </dl>
          )}
        </div>

        <div className="border-t border-(--border-soft) pt-4">
          <h3 className="text-xs font-semibold">Navigate</h3>
          <div className="mt-2 grid grid-cols-2 gap-2">
            <a
              href="/app/resources"
              className={buttonVariants({ variant: "outline", size: "sm" })}
            >
              <Boxes className="size-3.5" />
              Resources
            </a>
            <a
              href={analyticsHref()}
              className={buttonVariants({ variant: "outline", size: "sm" })}
            >
              <Gauge className="size-3.5" />
              Analytics
            </a>
            {canReadMembers && (
              <a
                href="/app/members"
                className={buttonVariants({ variant: "outline", size: "sm" })}
              >
                <Users className="size-3.5" />
                Members
              </a>
            )}
            <a
              href="/app/secrets"
              className={buttonVariants({ variant: "outline", size: "sm" })}
            >
              <KeyRound className="size-3.5" />
              Tokens
            </a>
            {canReadTaxonomy && (
              <>
                <a
                  href="/app/roles"
                  className={buttonVariants({ variant: "outline", size: "sm" })}
                >
                  <Shield className="size-3.5" />
                  Roles
                </a>
                <a
                  href="/app/tags"
                  className={buttonVariants({ variant: "outline", size: "sm" })}
                >
                  <Tags className="size-3.5" />
                  Tags
                </a>
              </>
            )}
            {canReadSettings && (
              <Button
                variant="outline"
                size="sm"
                className="col-span-2"
                onClick={onOpenSettings}
              >
                Project settings
              </Button>
            )}
          </div>
        </div>
      </CardContent>
    </Card>
  )
}

function FeedbackPanel({
  feedback,
  loading = false,
  announceLoading = true,
}: {
  feedback: DashboardSummary["feedback"] | undefined
  loading?: boolean
  announceLoading?: boolean
}) {
  const distribution = feedback
    ? [
        { rating: 5, count: feedback.distribution.rating_5 },
        { rating: 4, count: feedback.distribution.rating_4 },
        { rating: 3, count: feedback.distribution.rating_3 },
        { rating: 2, count: feedback.distribution.rating_2 },
        { rating: 1, count: feedback.distribution.rating_1 },
      ]
    : []

  return (
    <section
      aria-labelledby="dashboard-feedback"
      className="border-t border-(--border-soft) pt-4"
    >
      <div className="flex items-center justify-between gap-2">
        <h3 id="dashboard-feedback" className="text-xs font-semibold">
          Feedback
        </h3>
        {loading ? (
          <Skeleton className="h-5 w-20" />
        ) : (
          <Badge>
            {feedback?.scope === "project"
              ? "Project scope"
              : feedback?.scope === "owned_resources"
                ? "Owned resources"
                : "Not reported"}
          </Badge>
        )}
      </div>
      {loading ? (
        <LoadingState label="Loading feedback" announce={announceLoading} className="mt-3 grid gap-3">
          <div className="grid grid-cols-3 gap-2">
            {Array.from({ length: 3 }, (_, index) => (
              <div key={index} className="rounded-lg border border-(--border-soft) px-2 py-2">
                <Skeleton className="h-2.5 w-12" />
                <Skeleton className="mt-2 h-4 w-16 max-w-full" />
              </div>
            ))}
          </div>
          {Array.from({ length: 5 }, (_, index) => (
            <div key={index} className="flex items-center gap-2">
              <Skeleton className="h-2.5 w-9" />
              <Skeleton className="h-1.5 flex-1 rounded-full" />
              <Skeleton className="h-2.5 w-5" />
            </div>
          ))}
        </LoadingState>
      ) : feedback ? (
        <div className="mt-3 grid gap-3">
          <div className="grid grid-cols-3 gap-2">
            <FeedbackDatum
              label="Average"
              value={
                feedback.average_rating == null
                  ? "Not reported"
                  : `${feedback.average_rating.toFixed(1)} / 5`
              }
              icon={Star}
            />
            <FeedbackDatum
              label="Ratings"
              value={feedback.count.toLocaleString()}
            />
            <FeedbackDatum
              label="Positive"
              value={
                feedback.positive_percent == null
                  ? "Not reported"
                  : `${feedback.positive_percent.toFixed(1)}%`
              }
              hint={`${feedback.positive_count.toLocaleString()} ratings`}
            />
          </div>
          {feedback.count > 0 ? (
            <div
              aria-label={`Feedback distribution: ${distribution
                .map((item) => `${item.rating} stars ${item.count}`)
                .join(", ")}`}
              className="grid gap-1"
            >
              {distribution.map((item) => (
                <div
                  key={item.rating}
                  className="grid grid-cols-[2.25rem_minmax(0,1fr)_2rem] items-center gap-2 text-[0.65rem]"
                >
                  <span className="text-(--color-text-subtle)">
                    {item.rating} star
                  </span>
                  <div className="h-1.5 overflow-hidden rounded-full bg-(--bg-key)">
                    <div
                      className="h-full rounded-full bg-(--color-accent)"
                      style={{
                        width: `${Math.min(
                          100,
                          (item.count / feedback.count) * 100,
                        )}%`,
                      }}
                    />
                  </div>
                  <span className="text-right font-medium tabular-nums">
                    {item.count.toLocaleString()}
                  </span>
                </div>
              ))}
            </div>
          ) : (
            <p className="text-xs text-(--color-text-subtle)">
              No feedback has been recorded in this scope yet.
            </p>
          )}
          <p className="text-[0.65rem] leading-relaxed text-(--color-text-subtle)">
            {feedback.scope === "project"
              ? "Project scope. "
              : "Owned-resource scope; member identities are not exposed here. "}
            Current aggregate; not affected by the selected time range.
          </p>
        </div>
      ) : (
        <p className="mt-2 text-xs text-(--color-text-subtle)">
          Feedback is unavailable with the current project snapshot.
        </p>
      )}
    </section>
  )
}

function FeedbackDatum({
  label,
  value,
  hint,
  icon: Icon,
}: {
  label: string
  value: string
  hint?: string
  icon?: LucideIcon
}) {
  return (
    <div className="min-w-0 rounded-lg border border-(--border-soft) px-2 py-2">
      <div className="flex items-center gap-1 text-[0.62rem] text-(--color-text-subtle)">
        {Icon && <Icon className="size-3" />}
        <span>{label}</span>
      </div>
      <div
        className="mt-1 truncate text-xs font-semibold tabular-nums"
        title={value}
      >
        {value}
      </div>
      {hint && (
        <div className="truncate text-[0.6rem] text-(--color-text-subtle)">
          {hint}
        </div>
      )}
    </div>
  )
}

function WorkspaceDatum({
  label,
  value,
  icon: Icon,
}: {
  label: string
  value: string
  icon: LucideIcon
}) {
  return (
    <div className="flex items-center gap-2">
      <span className="grid size-6 shrink-0 place-items-center rounded-md bg-(--bg-key) text-(--color-text-subtle)">
        <Icon className="size-3" />
      </span>
      <dt className="min-w-0 flex-1 text-(--color-text-muted)">{label}</dt>
      <dd className="shrink-0 font-medium text-(--color-text)">{value}</dd>
    </div>
  )
}
