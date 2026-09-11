import { useMutation, useQuery } from "@tanstack/react-query"
import { useState } from "react"

import { formatMicrosAsUsd, parseUsdToMicros } from "@/features/spend/lib/usd"
import {
  api,
  type LimitPeriod,
  type LimitScope,
  type SpendLimitView,
} from "@/shared/api/client"
import { PRIMARY_ROLE_LABELS } from "@/shared/constants/member"
import { Button } from "@/shared/ui/button"
import { Dialog } from "@/shared/ui/dialog"
import { ErrorState } from "@/shared/ui/empty-state"
import { Input } from "@/shared/ui/input"
import { Label } from "@/shared/ui/label"
import { Select } from "@/shared/ui/select"
import { Switch } from "@/shared/ui/switch"

const SCOPES: { value: LimitScope; label: string }[] = [
  { value: "project", label: "Whole project" },
  { value: "member", label: "One member" },
  { value: "role", label: "Everyone with a role" },
]

const PERIODS: { value: LimitPeriod; label: string }[] = [
  { value: "day", label: "Each day" },
  { value: "week", label: "Each week (from Monday)" },
  { value: "month", label: "Each calendar month" },
]

const ROLES: { value: string; label: string }[] = (
  Object.keys(PRIMARY_ROLE_LABELS) as (keyof typeof PRIMARY_ROLE_LABELS)[]
).map((role) => ({ value: role, label: PRIMARY_ROLE_LABELS[role] }))

export function SpendLimitDialog({
  limit,
  onClose,
  onSaved,
}: {
  limit: SpendLimitView | "new"
  onClose: () => void
  onSaved: () => void
}) {
  const existing = limit === "new" ? null : limit
  const [scope, setScope] = useState<LimitScope>(existing?.scope ?? "project")
  const [subjectId, setSubjectId] = useState(existing?.subject_id ?? "")
  const [period, setPeriod] = useState<LimitPeriod>(existing?.period ?? "month")
  const [amount, setAmount] = useState(
    existing ? formatMicrosAsUsd(existing.limit_usd_micros) : "",
  )
  const [warnPercent, setWarnPercent] = useState(String(existing?.warn_percent ?? 80))
  const [enabled, setEnabled] = useState(existing?.enabled ?? true)
  const [formError, setFormError] = useState<string | null>(null)

  // Only fetched for the member scope; a role or project limit needs no list.
  const members = useQuery({
    queryKey: ["members", "spend-limit-picker"],
    queryFn: () => api.members({ limit: 200 }),
    enabled: scope === "member",
  })

  const save = useMutation({
    mutationFn: (micros: number) =>
      api.upsertSpendLimit({
        scope,
        subject_id: scope === "project" ? undefined : subjectId,
        period,
        limit_usd_micros: micros,
        warn_percent: Number(warnPercent),
        enabled,
      }),
    onSuccess: onSaved,
  })

  const submit = () => {
    setFormError(null)
    const micros = parseUsdToMicros(amount)
    if (micros === null) {
      setFormError(
        "Enter a plain dollar amount such as 250 or 12.50, with at most six decimals.",
      )
      return
    }
    if (scope !== "project" && !subjectId) {
      setFormError(
        scope === "member"
          ? "Choose the member this allowance applies to."
          : "Choose the role this allowance applies to.",
      )
      return
    }
    const warn = Number(warnPercent)
    if (!Number.isInteger(warn) || warn < 0 || warn > 100) {
      setFormError(
        "The warning threshold is a whole percentage from 0 to 100. Above 100 no warning could ever fire.",
      )
      return
    }
    save.mutate(micros)
  }

  const memberOptions =
    members.data?.items.map((member) => ({
      value: member.id,
      label: member.display_name,
    })) ?? []

  return (
    <Dialog
      open
      onClose={onClose}
      title={existing ? "Edit allowance" : "New allowance"}
      description={
        existing
          ? "Saving replaces every field, so an omitted threshold returns to its default."
          : "Conductor compares spend against this and reports the standing; it cannot block a call."
      }
      footer={
        <>
          <Button variant="outline" onClick={onClose} disabled={save.isPending}>
            Cancel
          </Button>
          <Button variant="gradient" onClick={submit} disabled={save.isPending}>
            {save.isPending ? "Saving…" : "Save allowance"}
          </Button>
        </>
      }
    >
      <div className="space-y-4">
        <div className="grid gap-4 sm:grid-cols-2">
          <div className="space-y-1.5">
            <Label htmlFor="spend-scope">Applies to</Label>
            <Select
              id="spend-scope"
              value={scope}
              options={SCOPES}
              disabled={existing !== null}
              onValueChange={(next) => {
                setScope(next)
                setSubjectId("")
              }}
            />
            {existing && (
              <p className="text-[11px] text-(--color-text-subtle)">
                Scope, subject and period identify an allowance. Remove this one and
                create another to change them.
              </p>
            )}
          </div>
          <div className="space-y-1.5">
            <Label htmlFor="spend-period">Resets</Label>
            <Select
              id="spend-period"
              value={period}
              options={PERIODS}
              disabled={existing !== null}
              onValueChange={setPeriod}
            />
          </div>
        </div>

        {scope === "member" && (
          <div className="space-y-1.5">
            <Label htmlFor="spend-member">Member</Label>
            <Select
              id="spend-member"
              value={subjectId}
              options={memberOptions}
              disabled={existing !== null || members.isLoading}
              placeholder={members.isLoading ? "Loading members…" : "Choose a member"}
              onValueChange={setSubjectId}
            />
          </div>
        )}
        {scope === "role" && (
          <div className="space-y-1.5">
            <Label htmlFor="spend-role">Role</Label>
            <Select
              id="spend-role"
              value={subjectId}
              options={ROLES}
              disabled={existing !== null}
              placeholder="Choose a role"
              onValueChange={setSubjectId}
            />
          </div>
        )}

        <div className="grid gap-4 sm:grid-cols-2">
          <div className="space-y-1.5">
            <Label htmlFor="spend-amount">Allowance (USD)</Label>
            <Input
              id="spend-amount"
              inputMode="decimal"
              placeholder="250.00"
              value={amount}
              onChange={(event) => setAmount(event.target.value)}
            />
          </div>
          <div className="space-y-1.5">
            <Label htmlFor="spend-warn">Warn at (%)</Label>
            <Input
              id="spend-warn"
              inputMode="numeric"
              value={warnPercent}
              onChange={(event) => setWarnPercent(event.target.value)}
            />
          </div>
        </div>

        <label className="flex items-center justify-between gap-3 rounded-lg border border-(--border-soft) px-3 py-2.5">
          <span>
            <span className="block text-sm font-medium">Evaluate this allowance</span>
            <span className="block text-xs text-(--color-text-subtle)">
              Switched off it stays configured but is not compared against spend.
            </span>
          </span>
          <Switch checked={enabled} onCheckedChange={setEnabled} />
        </label>

        <p className="text-xs text-(--color-text-subtle)">
          Usage Conductor could not price counts as zero here — a limit cannot charge for
          a cost nobody computed. Watch unpriced model calls alongside it.
        </p>

        {formError && <ErrorState message={formError} />}
        {save.error && (
          <ErrorState
            message={save.error instanceof Error ? save.error.message : "Save failed"}
          />
        )}
      </div>
    </Dialog>
  )
}
