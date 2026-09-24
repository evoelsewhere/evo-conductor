import { useQuery } from "@tanstack/react-query"
import { useNavigate } from "@tanstack/react-router"
import { ExternalLink, Settings2, Ticket } from "lucide-react"
import { useMemo, useState } from "react"

import { JiraRulesDialog } from "@/features/jira-report/components/jira-rules-dialog"
import { typeTone } from "@/features/jira-report/lib/task-color"
import { formatDuration, formatTokens } from "@/features/members/components/usage-formatters"
import { formatMicrosAsUsd } from "@/features/spend/lib/usd"
import { api, type TaskCostRow } from "@/shared/api/client"
import { PageFrame } from "@/shared/components/page-frame"
import { StatCardGrid } from "@/shared/components/stat-card"
import { useMinimumLoading } from "@/shared/hooks/use-minimum-loading"
import { cn } from "@/shared/lib/utils"
import { Badge } from "@/shared/ui/badge"
import { Button } from "@/shared/ui/button"
import { Card, CardHeader, CardTitle } from "@/shared/ui/card"
import { EmptyState, ErrorState } from "@/shared/ui/empty-state"
import { Select, type SelectOption } from "@/shared/ui/select"
import { LoadingState, Skeleton } from "@/shared/ui/skeleton"

type RangeDays = "7" | "14" | "30" | "90"
type ViewMode = "all" | "assignee" | "type" | "status"

const RANGE_OPTIONS: readonly SelectOption<RangeDays>[] = [
  { value: "7", label: "Last 7 days" },
  { value: "14", label: "Last 14 days" },
  { value: "30", label: "Last 30 days" },
  { value: "90", label: "Last 90 days" },
]

const VIEW_TABS: readonly { value: ViewMode; label: string }[] = [
  { value: "all", label: "All tasks" },
  { value: "assignee", label: "By assignee" },
  { value: "type", label: "By type" },
  { value: "status", label: "By Jira status" },
]

const PAGE_SIZE = 20

function fromForDays(days: RangeDays): string {
  const ms = Number(days) * 24 * 60 * 60 * 1000
  return new Date(Date.now() - ms).toISOString()
}

interface TaskNode {
  row: TaskCostRow
  children: TaskCostRow[]
  parentTitle: string | null
}

interface GroupRow {
  key: string
  tasks: number
  tracked: number
  calls: number
  tokens: number
  costMicros: number
}

export function JiraReportPage() {
  const navigate = useNavigate()
  const [days, setDays] = useState<RangeDays>("30")
  const [rulesOpen, setRulesOpen] = useState(false)
  const [view, setView] = useState<ViewMode>("all")
  const [page, setPage] = useState(1)
  const from = useMemo(() => fromForDays(days), [days])

  const report = useQuery({
    queryKey: ["task-cost-report", from],
    queryFn: () => api.taskCostReport({ from }),
  })
  const settings = useQuery({ queryKey: ["settings"], queryFn: () => api.settings() })
  const initialLoading = useMinimumLoading(report.isLoading)

  const rows = report.data?.rows ?? []
  const { roots } = useMemo(() => buildTree(rows), [rows])
  const jiraSiteUrl = settings.data?.jira.site_url ?? null

  const stats = useMemo(() => summarize(rows, report.data?.matched_tasks ?? 0, report.data?.unmatched_tasks ?? 0), [rows, report.data])
  const byAssignee = useMemo(
    () => groupRows(rows, (r) => r.assignee_display_name ?? "Unassigned in Jira"),
    [rows],
  )
  const byType = useMemo(() => groupRows(rows, (r) => r.resolved_type), [rows])
  const byStatus = useMemo(() => groupRows(rows, (r) => r.status || "Unknown"), [rows])

  const totalPages = Math.max(1, Math.ceil(roots.length / PAGE_SIZE))
  const pageRoots = roots.slice((page - 1) * PAGE_SIZE, page * PAGE_SIZE)

  const goToTask = (issueKey: string) => {
    void navigate({ to: "/app/jira-report/$issueKey", params: { issueKey } })
  }

  return (
    <PageFrame
      title="Jira report"
      subtitle="Every synced task. A Tracked task shows a real per-task slice, recorded by the jira-task-assistant plugin; a matched task nobody has started tracking yet shows zero rather than an estimate."
      action={
        <div className="flex items-center gap-2">
          <Button variant="outline" onClick={() => setRulesOpen(true)}>
            <Settings2 className="size-3.5" />
            Configure rules
          </Button>
          <Select
            value={days}
            onValueChange={(value) => {
              setDays(value)
              setPage(1)
            }}
            options={RANGE_OPTIONS}
            aria-label="Date range"
            className="w-40"
          />
        </div>
      }
    >
      {report.error && (
        <ErrorState
          className="mb-4"
          message={report.error instanceof Error ? report.error.message : "Failed to load report"}
        />
      )}

      <StatCardGrid className="mb-4">
        <DenseStatCard
          label="Tasks synced"
          value={stats.total}
          bar={stats.total > 0 ? stats.matched / stats.total : 0}
          sub={`${stats.epics} epics · ${stats.subtasks} subtasks`}
        />
        <DenseStatCard
          label="Matched"
          value={`${stats.matched} · ${stats.matchedPct}%`}
          sub={`${stats.tracked} Tracked · ${stats.notTrackedYet} not tracked yet`}
          hint="by configured Jira email"
        />
        <DenseStatCard
          label="Unmatched"
          value={`${stats.unmatched} · ${stats.unmatchedPct}%`}
          sub={stats.topUnmatched ? `top: ${stats.topUnmatched.name} (${stats.topUnmatched.count})` : "—"}
          hint="no member on file for assignee"
          warn
        />
        <DenseStatCard
          label="Tracked spend"
          value={`$${formatMicrosAsUsd(report.data?.matched_members_total_cost_usd_micros ?? 0)}`}
          sub={`${(report.data?.matched_members_total_calls ?? 0).toLocaleString()} calls · ${formatTokens(report.data?.matched_members_total_tokens ?? 0)} tokens`}
          hint="Tracked tasks only, not est."
        />
      </StatCardGrid>

      <Card>
        <CardHeader>
          <div>
            <CardTitle>Tasks</CardTitle>
            <p className="mt-1 text-xs text-(--color-text-muted)">
              Subtasks nest under their parent. A <strong className="font-medium text-(--color-text)">Tracked</strong> task's
              numbers are a real slice from the plugin's own activation calls. A task marked{" "}
              <strong className="font-medium text-(--color-text)">Not tracked yet</strong> is matched to a
              real member, but nobody has called the plugin's tool for that specific issue --
              it shows zero rather than guessing.
            </p>
          </div>
        </CardHeader>

        <div className="flex gap-1 border-b border-(--border-soft) px-4 pb-2">
          {VIEW_TABS.map((tab) => (
            <button
              key={tab.value}
              type="button"
              onClick={() => {
                setView(tab.value)
                setPage(1)
              }}
              className={cn(
                "rounded-md px-3 py-1.5 text-xs font-medium transition-colors",
                view === tab.value
                  ? "bg-(--color-accent-soft) text-(--color-accent)"
                  : "text-(--color-text-subtle) hover:text-(--color-text)",
              )}
            >
              {tab.label}
            </button>
          ))}
        </div>

        {initialLoading ? (
          <TaskListSkeleton />
        ) : report.error ? null : rows.length === 0 ? (
          <div className="p-4">
            <EmptyState
              icon={Ticket}
              title="No synced tasks yet"
              description="Jira may not be connected, or the sync job hasn't run yet — check Settings > Jira."
              className="border-0 bg-transparent py-8"
            />
          </div>
        ) : view === "all" ? (
          <>
            <div className="grid grid-cols-[2.1fr_0.9fr_0.9fr_0.9fr_0.7fr_0.6fr_0.8fr_0.8fr_24px] gap-0 px-4 py-2 text-[11px] tracking-wide text-(--color-text-subtle) uppercase">
              <span>Task</span>
              <span>Jira status</span>
              <span>Assignee</span>
              <span>Attribution</span>
              <span className="text-right">Duration</span>
              <span className="text-right">Calls</span>
              <span className="text-right">Tokens</span>
              <span className="text-right">Cost</span>
              <span />
            </div>
            <div className="divide-y divide-(--border-soft) border-t border-(--border-soft)">
              {pageRoots.map((node) => (
                <TaskGroup
                  key={node.row.issue_key}
                  node={node}
                  depth={0}
                  jiraSiteUrl={jiraSiteUrl}
                  onSelect={goToTask}
                />
              ))}
            </div>
            <div className="flex items-center justify-between gap-3 px-4 py-3">
              <span className="text-xs text-(--color-text-muted)">
                Click a row to open its full detail page. The small ↗ inside a row's title still opens Jira directly.
              </span>
              <div className="flex items-center gap-3">
                <span className="text-xs text-(--color-text-muted)">
                  {roots.length} tasks · page {page}/{totalPages}
                </span>
                <div className="flex gap-1">
                  <Button size="sm" variant="outline" disabled={page <= 1} onClick={() => setPage((p) => p - 1)}>
                    Prev
                  </Button>
                  <Button
                    size="sm"
                    variant="outline"
                    disabled={page >= totalPages}
                    onClick={() => setPage((p) => p + 1)}
                  >
                    Next
                  </Button>
                </div>
              </div>
            </div>
          </>
        ) : (
          <GroupTable
            rows={view === "assignee" ? byAssignee : view === "type" ? byType : byStatus}
            keyLabel={view === "assignee" ? "Assignee" : view === "type" ? "Type" : "Jira status"}
          />
        )}
      </Card>

      {rulesOpen && <JiraRulesDialog onClose={() => setRulesOpen(false)} />}
    </PageFrame>
  )
}

function summarize(rows: TaskCostRow[], matched: number, unmatched: number) {
  const total = rows.length
  const epics = rows.filter((r) => r.resolved_type.toLowerCase() === "epic").length
  const subtasks = rows.filter((r) => Boolean(r.parent_key)).length
  const tracked = rows.filter((r) => r.precise).length
  const notTrackedYet = rows.filter((r) => r.matched && !r.precise).length

  const unmatchedByAssignee = new Map<string, number>()
  for (const row of rows) {
    if (row.matched) continue
    const name = row.assignee_display_name ?? "Unassigned in Jira"
    unmatchedByAssignee.set(name, (unmatchedByAssignee.get(name) ?? 0) + 1)
  }
  let topUnmatched: { name: string; count: number } | null = null
  for (const [name, count] of unmatchedByAssignee) {
    if (!topUnmatched || count > topUnmatched.count) topUnmatched = { name, count }
  }

  return {
    total,
    matched,
    unmatched,
    epics,
    subtasks,
    tracked,
    notTrackedYet,
    topUnmatched,
    matchedPct: total > 0 ? Math.round((matched / total) * 100) : 0,
    unmatchedPct: total > 0 ? Math.round((unmatched / total) * 100) : 0,
  }
}

function groupRows(rows: TaskCostRow[], keyFn: (row: TaskCostRow) => string): GroupRow[] {
  const map = new Map<string, GroupRow>()
  for (const row of rows) {
    const key = keyFn(row)
    const group = map.get(key) ?? { key, tasks: 0, tracked: 0, calls: 0, tokens: 0, costMicros: 0 }
    group.tasks += 1
    if (row.precise) group.tracked += 1
    group.calls += row.calls
    group.tokens += row.total_tokens
    group.costMicros += row.total_cost_usd_micros
    map.set(key, group)
  }
  return [...map.values()].sort((a, b) => b.costMicros - a.costMicros)
}

function GroupTable({ rows, keyLabel }: { rows: GroupRow[]; keyLabel: string }) {
  return (
    <div>
      <div className="grid grid-cols-[1.8fr_0.7fr_0.7fr_0.6fr_0.8fr_0.7fr] gap-0 border-t border-(--border-soft) px-4 py-2 text-[11px] tracking-wide text-(--color-text-subtle) uppercase">
        <span>{keyLabel}</span>
        <span className="text-right">Tasks</span>
        <span className="text-right">Tracked</span>
        <span className="text-right">Calls</span>
        <span className="text-right">Tokens</span>
        <span className="text-right">Cost</span>
      </div>
      <div className="divide-y divide-(--border-soft)">
        {rows.map((group) => (
          <div
            key={group.key}
            className="grid grid-cols-[1.8fr_0.7fr_0.7fr_0.6fr_0.8fr_0.7fr] items-center gap-0 px-4 py-3 text-sm"
          >
            <span className="truncate">{group.key}</span>
            <span className="text-right tabular-nums">{group.tasks}</span>
            <span className="text-right tabular-nums text-(--color-success)">{group.tracked}</span>
            <span className="text-right tabular-nums">{group.calls.toLocaleString()}</span>
            <span className="text-right tabular-nums">{formatTokens(group.tokens)}</span>
            <span className="text-right tabular-nums font-medium">
              ${formatMicrosAsUsd(group.costMicros)}
            </span>
          </div>
        ))}
      </div>
    </div>
  )
}

function DenseStatCard({
  label,
  value,
  sub,
  hint,
  bar,
  warn,
}: {
  label: string
  value: React.ReactNode
  sub: string
  hint?: string
  bar?: number
  warn?: boolean
}) {
  return (
    <div
      className={cn(
        "rounded-xl border bg-(--bg-card) px-3.5 py-2.5",
        warn ? "border-(--color-warning)/35" : "border-(--border-card)",
      )}
    >
      <div className="flex items-baseline justify-between gap-2">
        <span className={cn("truncate text-xs", warn ? "text-(--color-warning)" : "text-(--color-text-muted)")}>
          {label}
        </span>
        <span className={cn("text-lg font-semibold tabular-nums", warn && "text-(--color-warning)")}>{value}</span>
      </div>
      {bar !== undefined && (
        <div className="mt-1.5 h-1 overflow-hidden rounded-full bg-(--color-warning)/40">
          <div className="h-full bg-(--color-success)" style={{ width: `${Math.round(bar * 100)}%` }} />
        </div>
      )}
      <div className="mt-1.5 text-[10.5px] text-(--color-text-subtle)">{sub}</div>
      {hint && <div className="mt-0.5 text-[10.5px] text-(--color-text-muted)/70">{hint}</div>}
    </div>
  )
}

function buildTree(rows: TaskCostRow[]): { roots: TaskNode[] } {
  const byKey = new Map<string, TaskCostRow>()
  for (const row of rows) byKey.set(row.issue_key, row)

  const childrenOf = new Map<string, TaskCostRow[]>()
  const roots: TaskCostRow[] = []

  for (const row of rows) {
    const parentKey = row.parent_key
    if (parentKey && byKey.has(parentKey)) {
      const list = childrenOf.get(parentKey) ?? []
      list.push(row)
      childrenOf.set(parentKey, list)
    } else {
      roots.push(row)
    }
  }

  const byCostDesc = (a: TaskCostRow, b: TaskCostRow) =>
    b.total_cost_usd_micros - a.total_cost_usd_micros

  const toNode = (row: TaskCostRow): TaskNode => ({
    row,
    children: (childrenOf.get(row.issue_key) ?? []).sort(byCostDesc),
    parentTitle: row.parent_key ? (byKey.get(row.parent_key)?.title ?? row.parent_key) : null,
  })

  return { roots: roots.sort(byCostDesc).map(toNode) }
}

function TaskGroup({
  node,
  depth,
  jiraSiteUrl,
  onSelect,
}: {
  node: TaskNode
  depth: number
  jiraSiteUrl: string | null
  onSelect: (issueKey: string) => void
}) {
  return (
    <div>
      <TaskRow
        row={node.row}
        parentTitle={node.parentTitle}
        depth={depth}
        jiraSiteUrl={jiraSiteUrl}
        onSelect={onSelect}
      />
      {node.children.map((child) => (
        <TaskRow
          key={child.issue_key}
          row={child}
          parentTitle={null}
          depth={depth + 1}
          jiraSiteUrl={jiraSiteUrl}
          onSelect={onSelect}
        />
      ))}
    </div>
  )
}

function TaskRow({
  row,
  parentTitle,
  depth,
  jiraSiteUrl,
  onSelect,
}: {
  row: TaskCostRow
  parentTitle: string | null
  depth: number
  jiraSiteUrl: string | null
  onSelect: (issueKey: string) => void
}) {
  const jiraUrl = jiraSiteUrl
    ? `${jiraSiteUrl.replace(/\/+$/, "")}/browse/${encodeURIComponent(row.issue_key)}`
    : null

  return (
    <div
      role="button"
      tabIndex={0}
      onClick={() => onSelect(row.issue_key)}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") onSelect(row.issue_key)
      }}
      className="grid grid-cols-[2.1fr_0.9fr_0.9fr_0.9fr_0.7fr_0.6fr_0.8fr_0.8fr_24px] items-center gap-0 px-4 py-3 text-left text-sm transition-colors hover:bg-(--bg-key) cursor-pointer"
      style={{ marginLeft: depth > 0 ? `${depth * 1.5}rem` : undefined }}
    >
      <div className="min-w-0 pr-2">
        <div className="flex flex-wrap items-center gap-2">
          <span className="font-mono text-xs text-(--color-text-subtle)">{row.issue_key}</span>
          <span className="truncate font-medium">{row.title}</span>
          {jiraUrl && (
            <a
              href={jiraUrl}
              target="_blank"
              rel="noreferrer"
              onClick={(e) => e.stopPropagation()}
              className="text-(--color-text-subtle) hover:text-(--color-text)"
              title="Open in Jira"
            >
              <ExternalLink className="size-3.5" />
            </a>
          )}
        </div>
        <div className="mt-1 flex flex-wrap items-center gap-1.5">
          <Badge tone={typeTone(row.resolved_type)}>{row.resolved_type}</Badge>
          {row.resolved_project && <Badge tone="info">{row.resolved_project}</Badge>}
          {parentTitle && (
            <span className="text-[11px] text-(--color-text-subtle)">subtask of {parentTitle}</span>
          )}
        </div>
        <StatusBreakdownLine row={row} />
      </div>
      <div className="text-xs text-(--color-text-subtle)">{row.status || "—"}</div>
      <div className="truncate text-xs text-(--color-text-subtle)">
        {row.assignee_display_name ?? "Unassigned in Jira"}
      </div>
      <div>
        {!row.matched && <Badge tone="warning">Unmatched</Badge>}
        {row.matched && (
          <Badge tone={row.precise ? "success" : "neutral"}>
            {row.precise ? "Tracked" : "Not tracked yet"}
          </Badge>
        )}
      </div>
      <div className="text-right text-xs text-(--color-text-subtle) tabular-nums">
        {row.total_duration_ms > 0 ? formatDuration(row.total_duration_ms) : "—"}
      </div>
      <div className="text-right tabular-nums">{row.calls.toLocaleString()}</div>
      <div className="text-right tabular-nums">{formatTokens(row.total_tokens)}</div>
      <div className="text-right font-medium tabular-nums">
        ${formatMicrosAsUsd(row.total_cost_usd_micros)}
      </div>
      <div className="text-right text-(--color-text-subtle)">›</div>
    </div>
  )
}

/** Prefers the rollup (this task plus its subtasks) over the task's own
 * split whenever the two actually differ -- a parent with tracked children
 * should read as "this whole effort", not just its own slice. Renders
 * nothing when there's no precise data to split at all. */
function StatusBreakdownLine({ row }: { row: TaskCostRow }) {
  const breakdown = row.rollup_by_status.length > 0 ? row.rollup_by_status : row.by_status
  if (breakdown.length === 0) return null
  const isRollup = breakdown === row.rollup_by_status
    && JSON.stringify(row.rollup_by_status) !== JSON.stringify(row.by_status)

  return (
    <div className="mt-1 flex flex-wrap items-center gap-x-3 gap-y-1 text-[11px] text-(--color-text-subtle)">
      {isRollup && <span className="italic">incl. subtasks:</span>}
      {breakdown.map((item) => (
        <span key={item.status}>
          <span className="text-(--color-text)">{item.status}</span>{" "}
          {item.total_tokens.toLocaleString()} tok
        </span>
      ))}
    </div>
  )
}

function TaskListSkeleton() {
  return (
    <LoadingState label="Loading task report">
      <div className="divide-y divide-(--border-soft)">
        {[60, 45, 52].map((width) => (
          <div key={width} className="flex items-center gap-3 px-4 py-3">
            <div className="min-w-0 flex-1 space-y-1.5">
              <Skeleton className="h-3.5" style={{ width: `${width}%` }} />
              <Skeleton className="h-2.5" style={{ width: `${width + 10}%` }} />
            </div>
            <Skeleton className="h-8 w-40" />
          </div>
        ))}
      </div>
    </LoadingState>
  )
}
