import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"
import { Plus, ShieldCheck, Trash2 } from "lucide-react"
import { useState } from "react"

import { AiPolicyDialog } from "@/features/ai-policy/components/ai-policy-dialog"
import { api, type AiPolicyView } from "@/shared/api/client"
import { PageFrame } from "@/shared/components/page-frame"
import { PRIMARY_ROLE_LABELS } from "@/shared/constants/member"
import { useMinimumLoading } from "@/shared/hooks/use-minimum-loading"
import { Badge } from "@/shared/ui/badge"
import { Button } from "@/shared/ui/button"
import { Card, CardHeader, CardList, CardTitle } from "@/shared/ui/card"
import { ConfirmDialog } from "@/shared/ui/dialog"
import { EmptyState, ErrorState } from "@/shared/ui/empty-state"
import { LoadingState, Skeleton } from "@/shared/ui/skeleton"

const AI_POLICIES_KEY = ["ai-policies"]

export function AiPolicyPage() {
  const qc = useQueryClient()
  const policies = useQuery({
    queryKey: AI_POLICIES_KEY,
    queryFn: () => api.aiPolicies(),
  })
  const [editing, setEditing] = useState<AiPolicyView | "new" | null>(null)
  const [pendingDelete, setPendingDelete] = useState<AiPolicyView | null>(null)
  const initialLoading = useMinimumLoading(policies.isLoading)

  const remove = useMutation({
    mutationFn: (policy: AiPolicyView) =>
      api.deleteAiPolicy(policy.scope, policy.subject_id || undefined),
    onSuccess: () => {
      setPendingDelete(null)
      void qc.invalidateQueries({ queryKey: AI_POLICIES_KEY })
    },
  })

  const rows = policies.data?.policies ?? []

  return (
    <PageFrame
      title="AI policy"
      subtitle="Which providers, models, and tools each project member's EvoFlux may use."
      action={
        <Button variant="gradient" onClick={() => setEditing("new")}>
          <Plus className="size-3.5" />
          New policy
        </Button>
      }
    >
      <p className="mb-4 rounded-lg border border-(--border-soft) bg-(--bg-key) px-3 py-2 text-sm text-(--color-text-muted)">
        A role policy <strong className="font-medium text-(--color-text)">narrows</strong>{" "}
        the project policy's allow-list — it can only restrict further, never widen past
        what the whole-project policy already allows. No policy at all means unrestricted,
        matching today's behavior until one is configured.
      </p>

      {policies.error && (
        <ErrorState
          className="mb-4"
          message={
            policies.error instanceof Error ? policies.error.message : "Failed to load policies"
          }
        />
      )}

      <Card>
        <CardHeader>
          <div>
            <CardTitle>Policies</CardTitle>
            <p className="mt-1 text-xs text-(--color-text-muted)">
              evoflux installations sync these periodically and enforce them at session
              start — a disallowed provider is rejected, never silently swapped.
            </p>
          </div>
        </CardHeader>

        {initialLoading ? (
          <PolicyListSkeleton />
        ) : policies.error ? null : rows.length === 0 ? (
          <div className="p-4">
            <EmptyState
              icon={ShieldCheck}
              title="No policies yet"
              description="Every provider and tool is unrestricted until you set one for the project or a role."
              className="border-0 bg-transparent py-8"
            />
          </div>
        ) : (
          <CardList>
            {rows.map((policy) => (
              <PolicyRow
                key={`${policy.scope}:${policy.subject_id}`}
                policy={policy}
                onEdit={() => setEditing(policy)}
                onDelete={() => setPendingDelete(policy)}
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
        <AiPolicyDialog
          policy={editing}
          onClose={() => setEditing(null)}
          onSaved={() => {
            setEditing(null)
            void qc.invalidateQueries({ queryKey: AI_POLICIES_KEY })
          }}
        />
      )}
      <ConfirmDialog
        open={pendingDelete !== null}
        title="Remove this policy?"
        description="Members it applied to fall back to the next-broadest policy, or unrestricted if none remains."
        confirmLabel="Remove policy"
        busy={remove.isPending}
        onClose={() => setPendingDelete(null)}
        onConfirm={() => pendingDelete && remove.mutate(pendingDelete)}
      />
    </PageFrame>
  )
}

function PolicyRow({
  policy,
  onEdit,
  onDelete,
}: {
  policy: AiPolicyView
  onEdit: () => void
  onDelete: () => void
}) {
  const subject =
    policy.scope === "project"
      ? "Everyone"
      : (PRIMARY_ROLE_LABELS[policy.subject_id as keyof typeof PRIMARY_ROLE_LABELS] ??
        policy.subject_id)

  return (
    <div className="flex flex-wrap items-center gap-3 px-4 py-3">
      <div className="min-w-0 flex-1">
        <div className="flex flex-wrap items-center gap-2">
          <span className="text-sm font-medium">{subject}</span>
          <Badge tone="neutral">{policy.scope === "project" ? "Whole project" : "Role"}</Badge>
        </div>
        <div className="mt-1 flex flex-wrap gap-x-4 gap-y-0.5 text-xs text-(--color-text-subtle)">
          <span>
            Providers:{" "}
            {policy.allowed_providers.length ? policy.allowed_providers.join(", ") : "any"}
          </span>
          <span>
            Tools: {policy.allowed_tools.length ? policy.allowed_tools.join(", ") : "any"}
          </span>
          {policy.default_model && <span>Default: {policy.default_model}</span>}
        </div>
      </div>

      <div className="flex items-center gap-1">
        <Button size="sm" variant="ghost" onClick={onEdit}>
          Edit
        </Button>
        <Button
          size="sm"
          variant="ghost"
          aria-label={`Remove the ${policy.scope} policy for ${subject}`}
          onClick={onDelete}
        >
          <Trash2 className="size-3.5" />
        </Button>
      </div>
    </div>
  )
}

function PolicyListSkeleton() {
  return (
    <LoadingState label="Loading policies">
      <div className="divide-y divide-(--border-soft)">
        {[52, 40].map((width) => (
          <div key={width} className="flex items-center gap-3 px-4 py-3">
            <div className="min-w-0 flex-1 space-y-1.5">
              <Skeleton className="h-3.5" style={{ width: `${width}%` }} />
              <Skeleton className="h-2.5" style={{ width: `${width + 12}%` }} />
            </div>
            <Skeleton className="size-8" />
          </div>
        ))}
      </div>
    </LoadingState>
  )
}
