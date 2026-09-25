import { useQuery } from "@tanstack/react-query"
import { Link, useParams } from "@tanstack/react-router"
import { ArrowLeft, ExternalLink } from "lucide-react"
import { useMemo, useState } from "react"

import { typeTone } from "@/features/jira-report/lib/task-color"
import { ModelDonutChart, TokenTrendChart } from "@/features/members/components/usage-charts"
import { formatDuration, formatTokens } from "@/features/members/components/usage-formatters"
import { formatMicrosAsUsd } from "@/features/spend/lib/usd"
import {
  api,
  type ModelUsageBreakdown,
  type StatusTokenBreakdown,
  type TaskActivityItem,
  type TaskCostRow,
} from "@/shared/api/client"
import { PageFrame } from "@/shared/components/page-frame"
import { useMinimumLoading } from "@/shared/hooks/use-minimum-loading"
import { cn } from "@/shared/lib/utils"
import { Badge } from "@/shared/ui/badge"
import { Button } from "@/shared/ui/button"
import { Card, CardHeader, CardTitle } from "@/shared/ui/card"
import { EmptyState, ErrorState } from "@/shared/ui/empty-state"
import { LoadingState, Skeleton } from "@/shared/ui/skeleton"

type SubTab = "requests" | "subtasks" | "log" | "status"

const SUB_TABS: readonly { value: SubTab; label: string }[] = [
  { value: "requests", label: "Requests" },
  { value: "subtasks", label: "Subtasks" },
  { value: "log", label: "Activity log" },
  { value: "status", label: "By status" },
]

const REQUEST_PAGE_SIZE = 25

function fromForDays(days: number): string {
  return new Date(Date.now() - days * 24 * 60 * 60 * 1000).toISOString()
}

export function JiraTaskDetailPage() {
  const { issueKey } = useParams({ strict: false }) as { issueKey: string }
  const [subTab, setSubTab] = useState<SubTab>("requests")
  const [requestPage, setRequestPage] = useState(1)
  const from = useMemo(() => fromForDays(30), [])

  const report = useQuery({
    queryKey: ["task-cost-report", from],
    queryFn: () => api.taskCostReport({ from }),
  })
  const settings = useQuery({ queryKey: ["settings"], queryFn: () => api.settings() })
  const activity = useQuery({
    queryKey: ["task-activity-detail", issueKey, from],
    queryFn: () => api.taskActivityDetail(issueKey, { from }),
  })

  const rows = report.data?.rows ?? []
  const row = rows.find((r) => r.issue_key === issueKey) ?? null
  const subtasks = rows.filter((r) => r.parent_key === issueKey)
  const jiraSiteUrl = settings.data?.jira.site_url ?? null
  const jiraUrl = jiraSiteUrl
    ? `${jiraSiteUrl.replace(/\/+$/, "")}/browse/${encodeURIComponent(issueKey)}`
    : null

  const initialLoading = useMinimumLoading(report.isLoading)
  const items = activity.data?.items ?? []

  const daily = useMemo(() => buildDaily(items), [items])
  const models = useMemo(() => buildModelBreakdown(items), [items])
  const breakdown = row ? (row.rollup_by_status.length > 0 ? row.rollup_by_status : row.by_status) : []

  const totalPages = Math.max(1, Math.ceil(items.length / REQUEST_PAGE_SIZE))
  const pageItems = items.slice((requestPage - 1) * REQUEST_PAGE_SIZE, requestPage * REQUEST_PAGE_SIZE)

  return (
    <PageFrame title={row ? `${row.issue_key} — ${row.title}` : "Task detail"}>
      <Link
        to="/app/jira-report"
        className="mb-3 inline-flex items-center gap-1 text-xs text-(--color-text-muted) hover:text-(--color-text)"
      >
        <ArrowLeft className="size-3.5" />
        All tasks
      </Link>

      {report.error && !initialLoading && (
        <ErrorState className="mb-4" message={report.error.message} />
      )}

      {initialLoading ? (
        <TaskDetailSkeleton />
      ) : !row ? (
        <EmptyState
          title="Task not found"
          description="This issue may have stopped syncing, or fallen outside the last 30 days."
          className="border-0 bg-transparent py-10"
        />
      ) : (
        <>
          <div className="mb-4 flex flex-wrap items-start justify-between gap-3">
            <div className="min-w-0">
              <div className="flex flex-wrap items-center gap-2">
                <Badge tone={typeTone(row.resolved_type)}>{row.resolved_type}</Badge>
                {row.resolved_project && <Badge tone="info">{row.resolved_project}</Badge>}
                {!row.matched && <Badge tone="warning">Unmatched</Badge>}
                {row.matched && (
                  <Badge tone={row.precise ? "success" : "neutral"}>
                    {row.precise ? "Tracked" : "Not tracked yet"}
                  </Badge>
                )}
                <span className="text-xs text-(--color-text-subtle)">Jira status: {row.status || "—"}</span>
              </div>
              <div className="mt-1 text-sm text-(--color-text-subtle)">
                {row.assignee_display_name ?? "Unassigned in Jira"}
              </div>
            </div>
            {jiraUrl && (
              <a href={jiraUrl} target="_blank" rel="noreferrer">
                <Button variant="outline">
                  <ExternalLink className="size-3.5" />
                  Open in Jira
                </Button>
              </a>
            )}
          </div>

          <div className="mb-4 grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-6">
            <Stat label="Input tokens" value={formatTokens(row.tokens_in)} />
            <Stat label="Output tokens" value={formatTokens(row.tokens_out)} />
            <Stat
              label="Cache tokens"
              value={formatTokens(row.cache_read_tokens + row.cache_write_tokens)}
              hint={row.cache_savings_usd_micros > 0 ? `$${formatMicrosAsUsd(row.cache_savings_usd_micros)} saved` : undefined}
            />
            <Stat label="Total time" value={row.total_duration_ms > 0 ? formatDuration(row.total_duration_ms) : "—"} />
            <Stat label="Cost" value={`$${formatMicrosAsUsd(row.total_cost_usd_micros)}`} />
            <Stat label="Requests" value={row.calls.toLocaleString()} />
          </div>

          {breakdown.length > 0 && (
            <div className="mb-4 rounded-xl border border-(--border-card) bg-(--bg-card) p-4">
              <div className="mb-2 text-xs font-medium tracking-wide text-(--color-text-muted) uppercase">
                Usage by Jira status
              </div>
              <div className="flex flex-wrap gap-4">
                {breakdown.map((entry) => (
                  <div key={entry.status} className="flex items-center gap-2 text-xs">
                    <span className="text-(--color-text)">{entry.status}</span>
                    <span className="text-(--color-text-subtle)">{formatTokens(entry.total_tokens)} tok</span>
                  </div>
                ))}
              </div>
            </div>
          )}

          {row.precise && (
            <div className="mb-4 grid gap-4 lg:grid-cols-2">
              <TokenTrendChart daily={daily} />
              <ModelDonutChart models={models} />
            </div>
          )}

          <Card>
            <CardHeader>
              <CardTitle>Details</CardTitle>
            </CardHeader>
            <div className="flex gap-1 border-b border-(--border-soft) px-4 pb-2">
              {SUB_TABS.map((tab) => (
                <button
                  key={tab.value}
                  type="button"
                  onClick={() => {
                    setSubTab(tab.value)
                    if (tab.value === "requests") setRequestPage(1)
                  }}
                  className={cn(
                    "rounded-md px-3 py-1.5 text-xs font-medium transition-colors",
                    subTab === tab.value
                      ? "bg-(--color-accent-soft) text-(--color-accent)"
                      : "text-(--color-text-subtle) hover:text-(--color-text)",
                  )}
                >
                  {tab.label}
                </button>
              ))}
            </div>

            {subTab === "requests" && (
              <RequestsTab
                loading={activity.isLoading}
                error={activity.error instanceof Error ? activity.error.message : null}
                precise={row.precise}
                items={pageItems}
                total={items.length}
                page={requestPage}
                totalPages={totalPages}
                onPage={setRequestPage}
                matchedUserId={row.matched_user_id}
              />
            )}
            {subTab === "subtasks" && <SubtasksTab subtasks={subtasks} />}
            {subTab === "log" && <ActivityLogTab items={items} />}
            {subTab === "status" && <StatusTab breakdown={breakdown} />}
          </Card>
        </>
      )}
    </PageFrame>
  )
}

function Stat({ label, value, hint }: { label: string; value: string; hint?: string }) {
  return (
    <div className="rounded-xl border border-(--border-card) bg-(--bg-card) px-3.5 py-2.5">
      <div className="text-lg font-semibold tabular-nums">{value}</div>
      <div className="mt-0.5 text-[11px] text-(--color-text-muted)">{label}</div>
      {hint && <div className="mt-0.5 text-[11px] text-(--color-success)">{hint}</div>}
    </div>
  )
}

function RequestsTab({
  loading,
  error,
  precise,
  items,
  total,
  page,
  totalPages,
  onPage,
  matchedUserId,
}: {
  loading: boolean
  error: string | null
  precise: boolean
  items: TaskActivityItem[]
  total: number
  page: number
  totalPages: number
  onPage: (updater: (p: number) => number) => void
  matchedUserId: string | null
}) {
  if (!precise) {
    return (
      <EmptyState
        title="No per-request detail for this task"
        description="This task's numbers are the assignee's whole-period estimate -- the jira-task-assistant plugin hasn't recorded an activation for this specific issue yet."
        className="border-0 bg-transparent py-10"
      />
    )
  }
  if (loading) {
    return (
      <LoadingState label="Loading requests">
        <div className="space-y-2 p-4">
          <Skeleton className="h-10 w-full" />
          <Skeleton className="h-10 w-full" />
          <Skeleton className="h-10 w-full" />
        </div>
      </LoadingState>
    )
  }
  if (error) return <ErrorState className="m-4" message={error} />
  if (total === 0) {
    return (
      <EmptyState
        title="No requests in this range"
        description="Nothing counted toward this task in the last 30 days."
        className="border-0 bg-transparent py-10"
      />
    )
  }

  return (
    <div>
      <div className="grid grid-cols-[0.9fr_0.9fr_0.7fr_1.1fr_0.6fr_0.9fr_0.6fr_0.7fr_0.6fr] gap-0 px-4 py-2 text-[11px] tracking-wide text-(--color-text-subtle) uppercase">
        <span>When</span>
        <span>Session</span>
        <span>Agent</span>
        <span>Provider:model</span>
        <span className="text-right">Duration</span>
        <span className="text-right">Input / output</span>
        <span className="text-right">Cache hit</span>
        <span className="text-right">Cost</span>
        <span>Status</span>
      </div>
      <div className="divide-y divide-(--border-soft) border-t border-(--border-soft)">
        {items.map((item, index) => (
          <RequestRow key={item.request_id ?? index} item={item} matchedUserId={matchedUserId} />
        ))}
      </div>
      <div className="flex items-center justify-between gap-3 px-4 py-3">
        <span className="text-xs text-(--color-text-muted)">
          {total} requests · page {page}/{totalPages}
        </span>
        <div className="flex gap-1">
          <Button size="sm" variant="outline" disabled={page <= 1} onClick={() => onPage((p) => p - 1)}>
            Prev
          </Button>
          <Button size="sm" variant="outline" disabled={page >= totalPages} onClick={() => onPage((p) => p + 1)}>
            Next
          </Button>
        </div>
      </div>
    </div>
  )
}

function RequestRow({ item, matchedUserId }: { item: TaskActivityItem; matchedUserId: string | null }) {
  const cacheBase = item.tokens_in + item.cache_read_tokens
  const cacheHitPct = cacheBase > 0 ? Math.round((item.cache_read_tokens / cacheBase) * 100) : null
  const content = (
    <>
      <div>
        <div>{new Date(item.occurred_at).toLocaleString()}</div>
        <div className="mt-0.5">
          <Badge tone="info">{item.jira_status}</Badge>
        </div>
      </div>
      <div className="truncate font-mono text-xs text-(--color-text-subtle)">
        {item.session_id ? `${item.session_id.slice(0, 8)}…${item.session_id.slice(-4)}` : "—"}
      </div>
      <div className="truncate text-(--color-text-subtle)">{item.agent_name ?? "—"}</div>
      <div className="truncate">
        {item.provider ?? "?"}:{item.model ?? "?"}
      </div>
      <div className="text-right text-(--color-text-subtle) tabular-nums">{formatDuration(item.duration_ms)}</div>
      <div className="text-right text-(--color-text-subtle) tabular-nums">
        {formatTokens(item.tokens_in)} / {formatTokens(item.tokens_out)}
      </div>
      <div className="text-right text-(--color-text-subtle) tabular-nums">
        {cacheHitPct === null ? "—" : `${cacheHitPct}%`}
      </div>
      <div className="text-right font-medium tabular-nums">${formatMicrosAsUsd(item.total_cost_usd_micros)}</div>
      <div>
        <Badge tone={item.status === "success" ? "success" : "danger"}>{item.status}</Badge>
      </div>
    </>
  )

  const gridClass =
    "grid grid-cols-[0.9fr_0.9fr_0.7fr_1.1fr_0.6fr_0.9fr_0.6fr_0.7fr_0.6fr] items-center gap-0 px-4 py-3 text-sm"

  if (matchedUserId && item.request_id) {
    return (
      <Link
        to="/app/members/$userId/activity/$requestId"
        params={{ userId: matchedUserId, requestId: item.request_id }}
        className={cn(gridClass, "transition-colors hover:bg-(--bg-key)")}
      >
        {content}
      </Link>
    )
  }
  return <div className={gridClass}>{content}</div>
}

function SubtasksTab({ subtasks }: { subtasks: TaskCostRow[] }) {
  if (subtasks.length === 0) {
    return (
      <EmptyState
        title="No subtasks"
        description="This issue has no synced child issues in the current window."
        className="border-0 bg-transparent py-10"
      />
    )
  }
  return (
    <div>
      <div className="grid grid-cols-[2.2fr_0.9fr_0.9fr_0.6fr_0.8fr_0.7fr] gap-0 px-4 py-2 text-[11px] tracking-wide text-(--color-text-subtle) uppercase">
        <span>Subtask</span>
        <span>Jira status</span>
        <span>Attribution</span>
        <span className="text-right">Calls</span>
        <span className="text-right">Tokens</span>
        <span className="text-right">Cost</span>
      </div>
      <div className="divide-y divide-(--border-soft) border-t border-(--border-soft)">
        {subtasks.map((s) => (
          <Link
            key={s.issue_key}
            to="/app/jira-report/$issueKey"
            params={{ issueKey: s.issue_key }}
            className="grid grid-cols-[2.2fr_0.9fr_0.9fr_0.6fr_0.8fr_0.7fr] items-center gap-0 px-4 py-3 text-sm transition-colors hover:bg-(--bg-key)"
          >
            <div className="min-w-0">
              <span className="font-mono text-xs text-(--color-text-subtle)">{s.issue_key}</span>
              <span className="ml-2 truncate font-medium">{s.title}</span>
            </div>
            <div className="text-xs text-(--color-text-subtle)">{s.status || "—"}</div>
            <div>
              {!s.matched && <Badge tone="warning">Unmatched</Badge>}
              {s.matched && (
                <Badge tone={s.precise ? "success" : "neutral"}>{s.precise ? "Tracked" : "Not tracked yet"}</Badge>
              )}
            </div>
            <div className="text-right tabular-nums">{s.calls.toLocaleString()}</div>
            <div className="text-right tabular-nums">{formatTokens(s.total_tokens)}</div>
            <div className="text-right font-medium tabular-nums">${formatMicrosAsUsd(s.total_cost_usd_micros)}</div>
          </Link>
        ))}
      </div>
    </div>
  )
}

function ActivityLogTab({ items }: { items: TaskActivityItem[] }) {
  if (items.length === 0) {
    return (
      <EmptyState
        title="Nothing recorded yet"
        description="A condensed, reverse-chronological digest of this task's model calls appears here once it's tracked."
        className="border-0 bg-transparent py-10"
      />
    )
  }
  return (
    <div className="max-h-96 overflow-y-auto px-4 py-2">
      {items.map((item, index) => (
        <div
          key={item.request_id ?? index}
          className="flex items-center gap-3 border-b border-(--border-soft) py-2 text-xs last:border-0"
        >
          <span className="w-36 shrink-0 text-(--color-text-subtle)">
            {new Date(item.occurred_at).toLocaleString()}
          </span>
          <span className="min-w-0 flex-1 truncate">
            {item.provider ?? "?"}:{item.model ?? "?"} · {item.agent_name ?? "evoflux"}
          </span>
          <span className="shrink-0 text-(--color-text-subtle) tabular-nums">
            {formatTokens(item.total_tokens)} tok
          </span>
          <span className="shrink-0 text-(--color-text-subtle) tabular-nums">
            {formatDuration(item.duration_ms)}
          </span>
        </div>
      ))}
    </div>
  )
}

function StatusTab({ breakdown }: { breakdown: StatusTokenBreakdown[] }) {
  if (breakdown.length === 0) {
    return (
      <EmptyState
        title="No status breakdown yet"
        description="Appears once this task has tracked usage to split by Jira workflow status."
        className="border-0 bg-transparent py-10"
      />
    )
  }
  return (
    <div>
      <div className="grid grid-cols-[1.6fr_0.8fr_0.9fr_0.7fr] gap-0 px-4 py-2 text-[11px] tracking-wide text-(--color-text-subtle) uppercase">
        <span>Jira status</span>
        <span className="text-right">Requests</span>
        <span className="text-right">Tokens</span>
        <span className="text-right">Cost</span>
      </div>
      <div className="divide-y divide-(--border-soft) border-t border-(--border-soft)">
        {breakdown.map((entry) => (
          <div key={entry.status} className="grid grid-cols-[1.6fr_0.8fr_0.9fr_0.7fr] items-center gap-0 px-4 py-3 text-sm">
            <span>{entry.status}</span>
            <span className="text-right tabular-nums">{entry.calls.toLocaleString()}</span>
            <span className="text-right tabular-nums">{formatTokens(entry.total_tokens)}</span>
            <span className="text-right font-medium tabular-nums">
              ${formatMicrosAsUsd(entry.total_cost_usd_micros)}
            </span>
          </div>
        ))}
      </div>
    </div>
  )
}

function buildDaily(items: TaskActivityItem[]) {
  const byDay = new Map<string, { requests: number; tokens_in: number; tokens_out: number; total_tokens: number; estimated_cost_usd_micros: number }>()
  for (const item of items) {
    const date = item.occurred_at.slice(0, 10)
    const entry = byDay.get(date) ?? { requests: 0, tokens_in: 0, tokens_out: 0, total_tokens: 0, estimated_cost_usd_micros: 0 }
    entry.requests += 1
    entry.tokens_in += item.tokens_in
    entry.tokens_out += item.tokens_out
    entry.total_tokens += item.total_tokens
    entry.estimated_cost_usd_micros += item.total_cost_usd_micros
    byDay.set(date, entry)
  }
  return [...byDay.entries()]
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([date, entry]) => ({ date, ...entry }))
}

function buildModelBreakdown(items: TaskActivityItem[]): ModelUsageBreakdown[] {
  const byModel = new Map<string, ModelUsageBreakdown>()
  for (const item of items) {
    const provider = item.provider ?? ""
    const model = item.model ?? "unknown"
    const key = `${provider}:${model}`
    const entry = byModel.get(key) ?? {
      provider,
      model,
      calls: 0,
      tokens_in: 0,
      tokens_out: 0,
      total_tokens: 0,
      cache_read_tokens: 0,
      cache_write_tokens: 0,
      estimated_cost_usd_micros: 0,
      cache_savings_usd_micros: 0,
      unpriced_calls: 0,
    }
    entry.calls += 1
    entry.tokens_in += item.tokens_in
    entry.tokens_out += item.tokens_out
    entry.total_tokens += item.total_tokens
    entry.cache_read_tokens += item.cache_read_tokens
    entry.cache_write_tokens += item.cache_write_tokens
    entry.estimated_cost_usd_micros += item.total_cost_usd_micros
    entry.cache_savings_usd_micros += item.cache_savings_usd_micros
    byModel.set(key, entry)
  }
  return [...byModel.values()].sort((a, b) => b.total_tokens - a.total_tokens)
}

function TaskDetailSkeleton() {
  return (
    <LoadingState label="Loading task detail">
      <div className="space-y-4">
        <Skeleton className="h-6 w-96" />
        <div className="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-6">
          {Array.from({ length: 6 }, (_, index) => (
            <Skeleton key={index} className="h-16 w-full" />
          ))}
        </div>
        <Skeleton className="h-64 w-full" />
      </div>
    </LoadingState>
  )
}
