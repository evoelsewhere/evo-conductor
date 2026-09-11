import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"
import { CircleDollarSign, Plus, RefreshCw, Trash2, Wallet } from "lucide-react"
import { useState } from "react"

import { SpendLimitDialog } from "@/features/spend/components/spend-limit-dialog"
import { formatMicrosAsUsd } from "@/features/spend/lib/usd"
import {
  api,
  type LimitPeriod,
  type LimitScope,
  type SpendLimitView,
} from "@/shared/api/client"
import { PageFrame } from "@/shared/components/page-frame"
import { StatCard, StatCardGrid } from "@/shared/components/stat-card"
import { useMinimumLoading } from "@/shared/hooks/use-minimum-loading"
import { Badge } from "@/shared/ui/badge"
import { Button } from "@/shared/ui/button"
import { Card, CardHeader, CardList, CardTitle } from "@/shared/ui/card"
import { ConfirmDialog } from "@/shared/ui/dialog"
import { EmptyState, ErrorState } from "@/shared/ui/empty-state"
import { LoadingState, Skeleton } from "@/shared/ui/skeleton"

const SCOPE_LABEL: Record<LimitScope, string> = {
  project: "Whole project",
  member: "Member",
  role: "Role",
}

const PERIOD_LABEL: Record<LimitPeriod, string> = {
  day: "per day",
  week: "per week",
  month: "per month",
}

const SPEND_LIMITS_KEY = ["spend-limits"]
const MODEL_PRICING_KEY = ["model-pricing"]

export function SpendPage() {
  const qc = useQueryClient()
  const limits = useQuery({
    queryKey: SPEND_LIMITS_KEY,
    queryFn: () => api.spendLimits(),
  })
  const [editing, setEditing] = useState<SpendLimitView | "new" | null>(null)
  const [pendingDelete, setPendingDelete] = useState<SpendLimitView | null>(null)
  const initialLoading = useMinimumLoading(limits.isLoading)

  const remove = useMutation({
    mutationFn: (limit: SpendLimitView) =>
      api.deleteSpendLimit(limit.scope, limit.period, limit.subject_id || undefined),
    onSuccess: () => {
      setPendingDelete(null)
      void qc.invalidateQueries({ queryKey: SPEND_LIMITS_KEY })
    },
  })

  const rows = limits.data?.limits ?? []
  const watched = rows.filter((limit) => limit.enabled)
  const overLimit = watched.filter((limit) => limit.status?.state === "exceeded")
  const warning = watched.filter((limit) => limit.status?.state === "warning")

  return (
    <PageFrame
      title="Spend"
      subtitle="Allowances Conductor watches, and the price catalog it values usage against."
      action={
        <Button variant="gradient" onClick={() => setEditing("new")}>
          <Plus className="size-3.5" />
          New limit
        </Button>
      }
    >
      {/* The single most important thing an admin can misunderstand here. */}
      <p className="mb-4 rounded-lg border border-(--border-soft) bg-(--bg-key) px-3 py-2 text-sm text-(--color-text-muted)">
        A limit <strong className="font-medium text-(--color-text)">detects, it does not enforce</strong>.
        EvoFlux calls model providers with its own credentials, so nothing here can stop a
        call from being made — not even revoking a connection token, which only cuts off
        sync and telemetry. These allowances produce a status to act on.
      </p>

      {limits.error && (
        <ErrorState
          className="mb-4"
          message={limits.error instanceof Error ? limits.error.message : "Failed to load limits"}
        />
      )}

      <StatCardGrid className="mb-4">
        <StatCard
          label="Watched allowances"
          value={watched.length}
          hint={`${rows.length - watched.length} disabled`}
          icon={Wallet}
        />
        <StatCard
          label="Over limit"
          value={overLimit.length}
          hint="this period"
          icon={CircleDollarSign}
          tone={overLimit.length > 0 ? "warning" : "success"}
        />
        <StatCard
          label="Near limit"
          value={warning.length}
          hint="past the warning threshold"
          icon={CircleDollarSign}
          tone={warning.length > 0 ? "warning" : "neutral"}
        />
      </StatCardGrid>

      <PricingCard />

      <Card className="mt-4">
        <CardHeader>
          <div>
            <CardTitle>Allowances</CardTitle>
            <p className="mt-1 text-xs text-(--color-text-muted)">
              One per scope, subject and period. Spend is summed with the same cost the
              dashboards show, over when Conductor received the telemetry.
            </p>
          </div>
        </CardHeader>

        {initialLoading ? (
          <LimitListSkeleton />
        ) : limits.error ? null : rows.length === 0 ? (
          <div className="p-4">
            <EmptyState
              icon={Wallet}
              title="No allowances yet"
              description="Set one for the whole project, a role, or a single member to see spend against it."
              className="border-0 bg-transparent py-8"
            />
          </div>
        ) : (
          <CardList>
            {rows.map((limit) => (
              <LimitRow
                key={`${limit.scope}:${limit.subject_id}:${limit.period}`}
                limit={limit}
                onEdit={() => setEditing(limit)}
                onDelete={() => setPendingDelete(limit)}
              />
            ))}
          </CardList>
        )}
      </Card>

      {remove.error && (
        <ErrorState
          className="mt-4"
          message={remove.error instanceof Error ? remove.error.message : "Delete failed"}
        />
      )}

      {editing && (
        <SpendLimitDialog
          limit={editing}
          onClose={() => setEditing(null)}
          onSaved={() => {
            setEditing(null)
            void qc.invalidateQueries({ queryKey: SPEND_LIMITS_KEY })
          }}
        />
      )}
      <ConfirmDialog
        open={pendingDelete !== null}
        title="Remove this allowance?"
        description="Spend keeps being recorded; only the allowance it was compared against goes away."
        confirmLabel="Remove allowance"
        busy={remove.isPending}
        onClose={() => setPendingDelete(null)}
        onConfirm={() => pendingDelete && remove.mutate(pendingDelete)}
      />
    </PageFrame>
  )
}

function LimitRow({
  limit,
  onEdit,
  onDelete,
}: {
  limit: SpendLimitView
  onEdit: () => void
  onDelete: () => void
}) {
  const subject =
    limit.scope === "project"
      ? "Everyone"
      : (limit.subject_label ?? limit.subject_id)
  const status = limit.status
  const tone =
    status?.state === "exceeded"
      ? "danger"
      : status?.state === "warning"
        ? "warning"
        : "success"

  return (
    <div className="flex flex-wrap items-center gap-3 px-4 py-3">
      <div className="min-w-0 flex-1">
        <div className="flex flex-wrap items-center gap-2">
          <span className="text-sm font-medium">{subject}</span>
          <Badge tone="neutral">{SCOPE_LABEL[limit.scope]}</Badge>
          <span className="text-xs text-(--color-text-subtle)">
            {PERIOD_LABEL[limit.period]}
          </span>
        </div>
        <div className="mt-1 text-xs text-(--color-text-subtle)">
          {status ? (
            <>
              ${formatMicrosAsUsd(status.spent_usd_micros)} of $
              {formatMicrosAsUsd(limit.limit_usd_micros)} · warns at {limit.warn_percent}%
            </>
          ) : (
            <>
              ${formatMicrosAsUsd(limit.limit_usd_micros)} allowance · warns at{" "}
              {limit.warn_percent}%
            </>
          )}
        </div>
      </div>

      {status ? (
        <div className="flex w-40 shrink-0 items-center gap-2">
          <div className="h-1.5 flex-1 overflow-hidden rounded-full bg-(--bg-key)">
            <div
              className="h-full rounded-full"
              style={{
                width: `${Math.min(status.used_percent, 100)}%`,
                background:
                  tone === "danger"
                    ? "var(--color-error)"
                    : tone === "warning"
                      ? "var(--color-warning)"
                      : "var(--color-success)",
              }}
            />
          </div>
          <span className="w-11 shrink-0 text-right text-xs tabular-nums">
            {status.used_percent}%
          </span>
        </div>
      ) : (
        // A disabled limit is configured but never evaluated, which is a
        // different thing from evaluating to zero.
        <Badge tone="neutral">Not evaluated</Badge>
      )}

      <div className="flex items-center gap-1">
        <Button size="sm" variant="ghost" onClick={onEdit}>
          Edit
        </Button>
        <Button
          size="sm"
          variant="ghost"
          aria-label={`Remove the ${SCOPE_LABEL[limit.scope].toLowerCase()} allowance for ${subject}`}
          onClick={onDelete}
        >
          <Trash2 className="size-3.5" />
        </Button>
      </div>
    </div>
  )
}

/**
 * Catalog state, and the two operator actions on it.
 *
 * Worth its own panel because an unpriced project reports zero spend, and zero
 * spend against a limit reads exactly like thrift.
 */
function PricingCard() {
  const qc = useQueryClient()
  const { data, isLoading, error } = useQuery({
    queryKey: MODEL_PRICING_KEY,
    queryFn: () => api.modelPricing(),
  })
  const [note, setNote] = useState<string | null>(null)

  const sync = useMutation({
    mutationFn: () => api.syncModelPricing(),
    onSuccess: (result) => {
      setNote(
        result.unchanged
          ? `Catalog ${result.version} was already current.`
          : `Synced ${result.model_count} models, ${result.priced_model_count} with prices; ${result.changed_model_count} rates changed.`,
      )
      void qc.invalidateQueries({ queryKey: MODEL_PRICING_KEY })
      void qc.invalidateQueries({ queryKey: SPEND_LIMITS_KEY })
    },
  })
  const reprice = useMutation({
    mutationFn: () => api.repriceModelCalls(),
    onSuccess: (report) => {
      setNote(
        `Examined ${report.examined} unpriced calls, priced ${report.priced_in_force}; ${report.left_unpriced} still have no rate.`,
      )
      void qc.invalidateQueries({ queryKey: SPEND_LIMITS_KEY })
    },
  })
  const busy = sync.isPending || reprice.isPending
  const failure = sync.error ?? reprice.error

  return (
    <Card>
      <CardHeader>
        <div>
          <CardTitle>Price catalog</CardTitle>
          <p className="mt-1 text-xs text-(--color-text-muted)">
            Conductor values usage from its own models.dev snapshot rather than trusting
            what each client reports.
          </p>
        </div>
        <div className="flex gap-2">
          <Button
            size="sm"
            variant="outline"
            disabled={busy || !data?.sync_enabled}
            onClick={() => {
              setNote(null)
              sync.mutate()
            }}
          >
            <RefreshCw className="size-3.5" />
            Sync now
          </Button>
          <Button
            size="sm"
            variant="outline"
            disabled={busy}
            onClick={() => {
              setNote(null)
              reprice.mutate()
            }}
          >
            Reprice
          </Button>
        </div>
      </CardHeader>

      <div className="space-y-2 px-4 pb-4 text-sm">
        {isLoading ? (
          <Skeleton className="h-4 w-64" />
        ) : error ? (
          <ErrorState
            message={error instanceof Error ? error.message : "Failed to load catalog"}
          />
        ) : !data?.catalog ? (
          <p className="text-(--color-text-muted)">
            No catalog has been synced, so nothing Conductor has ingested carries a price
            yet — spend will read as zero against every allowance until one is.
          </p>
        ) : (
          <dl className="grid gap-x-6 gap-y-1 text-xs sm:grid-cols-3">
            <Detail label="Version" value={data.catalog.version.slice(0, 12)} mono />
            <Detail
              label="Fetched"
              value={new Date(data.catalog.fetched_at).toLocaleString()}
            />
            <Detail label="Latest catalog size" value={`${data.catalog.model_count} models`} />
            <Detail
              label="Rates in force"
              value={`${data.priced_models} models`}
            />
          </dl>
        )}
        {data && !data.sync_enabled && (
          <p className="text-xs text-(--color-text-subtle)">
            Automatic and manual sync are switched off for this server
            (<code>CONDUCTOR_MODEL_PRICING_ENABLED</code>). Repricing still works from
            whatever rates are already stored.
          </p>
        )}
        {note && <p className="text-xs text-(--color-text-muted)">{note}</p>}
        {failure && (
          <ErrorState
            message={failure instanceof Error ? failure.message : "Operation failed"}
          />
        )}
      </div>
    </Card>
  )
}

function Detail({
  label,
  value,
  mono,
}: {
  label: string
  value: string
  mono?: boolean
}) {
  return (
    <div>
      <dt className="text-(--color-text-subtle)">{label}</dt>
      <dd className={mono ? "font-mono" : undefined}>{value}</dd>
    </div>
  )
}

function LimitListSkeleton() {
  return (
    <LoadingState label="Loading allowances">
      <div className="divide-y divide-(--border-soft)">
        {[52, 40, 46].map((width) => (
          <div key={width} className="flex items-center gap-3 px-4 py-3">
            <div className="min-w-0 flex-1 space-y-1.5">
              <Skeleton className="h-3.5" style={{ width: `${width}%` }} />
              <Skeleton className="h-2.5" style={{ width: `${width + 12}%` }} />
            </div>
            <Skeleton className="h-1.5 w-40" />
            <Skeleton className="size-8" />
          </div>
        ))}
      </div>
    </LoadingState>
  )
}
