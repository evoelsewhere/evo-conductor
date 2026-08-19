import { ArrowRight, Monitor } from "lucide-react"

import { dashboardInitials } from "@/features/dashboard/lib/dashboard-formatters"
import { formatTokens } from "@/features/members/components/usage-formatters"
import { formatEstimatedCost } from "@/features/resource-usage/components/resource-usage-formatters"
import type { ResourceUsageMember } from "@/shared/api/client"
import { PRIMARY_ROLE_LABELS } from "@/shared/constants/member"
import { Badge } from "@/shared/ui/badge"
import { buttonVariants } from "@/shared/ui/button"
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/shared/ui/card"
import { LoadingState, Skeleton } from "@/shared/ui/skeleton"
import {
  Table,
  TableBody,
  TableHead,
  TableRow,
  TableTd,
  TableTh,
  TableWrap,
} from "@/shared/ui/table"

export function MemberActivityPanel({
  members,
  loading,
  analyticsHref,
  announceLoading = true,
}: {
  members: ResourceUsageMember[]
  loading: boolean
  analyticsHref: (filters?: Record<string, string>) => string
  announceLoading?: boolean
}) {
  return (
    <Card>
      <CardHeader>
        <div>
          <CardTitle>Member activity</CardTitle>
          <CardDescription className="mt-0.5">
            Admin-only, server-received activity attributed to governed resources in the selected range.
          </CardDescription>
        </div>
        <a
          href={analyticsHref()}
          className={buttonVariants({ variant: "outline", size: "sm" })}
        >
          Inspect all activity
          <ArrowRight className="size-3.5" />
        </a>
      </CardHeader>
      <CardContent className="p-0">
        {loading ? (
          <MemberActivitySkeleton announce={announceLoading} />
        ) : members.length > 0 ? (
          <>
            <div className="hidden md:block">
              <TableWrap className="rounded-none border-0">
                <Table>
                  <TableHead>
                    <tr>
                      <TableTh>Member</TableTh>
                      <TableTh>Current role</TableTh>
                      <TableTh>Clients in range</TableTh>
                      <TableTh>Requests / uses</TableTh>
                      <TableTh>Model / tool calls</TableTh>
                      <TableTh>Tokens / estimate</TableTh>
                      <TableTh>Last received</TableTh>
                      <TableTh>
                        <span className="sr-only">Actions</span>
                      </TableTh>
                    </tr>
                  </TableHead>
                  <TableBody>
                    {members.map((member) => (
                      <TableRow key={member.user_id}>
                        <TableTd>
                          <MemberIdentity member={member} />
                        </TableTd>
                        <TableTd>
                          <Badge tone="neutral">
                            {PRIMARY_ROLE_LABELS[member.primary_role]}
                          </Badge>
                        </TableTd>
                        <TableTd className="tabular-nums">
                          {member.installations.toLocaleString()}
                        </TableTd>
                        <TableTd className="tabular-nums">
                          <strong className="font-medium">
                            {member.requests.toLocaleString()}
                          </strong>
                          <span className="block text-xs text-(--color-text-subtle)">
                            {member.resource_uses.toLocaleString()} uses
                          </span>
                        </TableTd>
                        <TableTd className="tabular-nums">
                          {member.model_calls.toLocaleString()} / {member.tool_calls.toLocaleString()}
                        </TableTd>
                        <TableTd className="tabular-nums">
                          <strong className="font-medium">
                            {formatTokens(member.total_tokens)}
                          </strong>
                          <span className="block text-xs text-(--color-text-subtle)">
                            {formatEstimatedCost(member.estimated_cost_usd_micros)} estimated
                          </span>
                        </TableTd>
                        <TableTd className="whitespace-nowrap text-xs text-(--color-text-muted)">
                          {formatLastReceived(member.last_received_at)}
                        </TableTd>
                        <TableTd>
                          <a
                            href={analyticsHref({ member_id: member.user_id })}
                            className={buttonVariants({ variant: "ghost", size: "sm" })}
                            aria-label={`Inspect governed activity for ${member.display_name}`}
                          >
                            Inspect
                            <ArrowRight className="size-3.5" />
                          </a>
                        </TableTd>
                      </TableRow>
                    ))}
                  </TableBody>
                </Table>
              </TableWrap>
            </div>

            <div className="divide-y divide-(--border-soft) md:hidden">
              {members.map((member) => (
                <div
                  key={member.user_id}
                  className="px-4 py-3"
                >
                  <div className="flex items-center gap-3">
                    <MemberIdentity member={member} />
                  </div>
                  <dl className="mt-3 grid grid-cols-2 gap-x-4 gap-y-2 text-xs">
                    <MobileDatum label="Role" value={PRIMARY_ROLE_LABELS[member.primary_role]} />
                    <MobileDatum label="Clients" value={member.installations.toLocaleString()} />
                    <MobileDatum label="Requests" value={member.requests.toLocaleString()} />
                    <MobileDatum label="Resource uses" value={member.resource_uses.toLocaleString()} />
                    <MobileDatum label="Model / tool" value={`${member.model_calls.toLocaleString()} / ${member.tool_calls.toLocaleString()}`} />
                    <MobileDatum label="Tokens" value={formatTokens(member.total_tokens)} />
                    <MobileDatum label="Estimated cost" value={formatEstimatedCost(member.estimated_cost_usd_micros)} />
                    <MobileDatum label="Last received" value={formatLastReceived(member.last_received_at)} />
                  </dl>
                  <a
                    href={analyticsHref({ member_id: member.user_id })}
                    className="mt-3 inline-flex items-center gap-1 text-xs font-medium text-(--color-accent) outline-none focus-visible:ring-2 focus-visible:ring-(--focus-ring)/35"
                  >
                    Inspect governed activity
                    <ArrowRight className="size-3.5" />
                  </a>
                </div>
              ))}
            </div>
          </>
        ) : (
          <div className="px-4 py-8 text-center">
            <Monitor className="mx-auto size-5 text-(--color-text-subtle)" />
            <p className="mt-2 text-sm font-medium">No member activity in this range</p>
            <p className="mt-1 text-xs text-(--color-text-subtle)">
              This table populates when an active member uses a governed resource and telemetry reaches Conductor.
            </p>
          </div>
        )}
      </CardContent>
    </Card>
  )
}

function MemberIdentity({ member }: { member: ResourceUsageMember }) {
  return (
    <div className="flex min-w-0 items-center gap-2.5">
      <span className="grid size-8 shrink-0 place-items-center rounded-full bg-(--color-accent-soft) text-[0.7rem] font-semibold text-(--color-accent)">
        {dashboardInitials(member.display_name)}
      </span>
      <span className="min-w-0">
        <a
          href={`/app/members/${member.user_id}`}
          className="block truncate font-medium outline-none hover:text-(--color-accent) focus-visible:ring-2 focus-visible:ring-(--focus-ring)/35"
        >
          {member.display_name}
        </a>
        <span className="block text-xs text-(--color-text-subtle)">
          Governed activity
        </span>
      </span>
    </div>
  )
}

function MobileDatum({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <dt className="text-(--color-text-subtle)">{label}</dt>
      <dd className="mt-0.5 font-medium tabular-nums">{value}</dd>
    </div>
  )
}

function MemberActivitySkeleton({ announce }: { announce: boolean }) {
  return (
    <LoadingState label="Loading member activity" announce={announce} className="grid gap-0">
      {Array.from({ length: 3 }, (_, index) => (
        <div key={index} className="flex items-center gap-3 border-b border-(--border-soft) px-4 py-3 last:border-b-0">
          <Skeleton className="size-8 shrink-0 rounded-full" />
          <div className="min-w-0 flex-1">
            <Skeleton className="h-3.5 w-32" />
            <Skeleton className="mt-1.5 h-3 w-24" />
          </div>
          <Skeleton className="hidden h-5 w-16 sm:block" />
          <Skeleton className="hidden h-4 w-20 md:block" />
          <Skeleton className="hidden h-4 w-24 lg:block" />
        </div>
      ))}
    </LoadingState>
  )
}

function formatLastReceived(value: string) {
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) return "Not reported"
  return new Intl.DateTimeFormat(undefined, {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  }).format(date)
}
