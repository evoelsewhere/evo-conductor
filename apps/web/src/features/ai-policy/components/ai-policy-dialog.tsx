import { useMutation } from "@tanstack/react-query"
import { useState } from "react"

import {
  api,
  type AiPolicyScope,
  type AiPolicyView,
} from "@/shared/api/client"
import { PRIMARY_ROLE_LABELS } from "@/shared/constants/member"
import { Button } from "@/shared/ui/button"
import { Dialog } from "@/shared/ui/dialog"
import { ErrorState } from "@/shared/ui/empty-state"
import { Input } from "@/shared/ui/input"
import { Label } from "@/shared/ui/label"
import { Select } from "@/shared/ui/select"

const SCOPES: { value: AiPolicyScope; label: string }[] = [
  { value: "project", label: "Whole project" },
  { value: "role", label: "Everyone with a role" },
]

const ROLES: { value: string; label: string }[] = (
  Object.keys(PRIMARY_ROLE_LABELS) as (keyof typeof PRIMARY_ROLE_LABELS)[]
).map((role) => ({ value: role, label: PRIMARY_ROLE_LABELS[role] }))

function splitList(value: string): string[] {
  return value
    .split(/[\s,]+/)
    .map((item) => item.trim())
    .filter(Boolean)
}

export function AiPolicyDialog({
  policy,
  onClose,
  onSaved,
}: {
  policy: AiPolicyView | "new"
  onClose: () => void
  onSaved: () => void
}) {
  const existing = policy === "new" ? null : policy
  const [scope, setScope] = useState<AiPolicyScope>(existing?.scope ?? "project")
  const [subjectId, setSubjectId] = useState(existing?.subject_id ?? "")
  const [defaultProvider, setDefaultProvider] = useState(existing?.default_provider ?? "")
  const [defaultModel, setDefaultModel] = useState(existing?.default_model ?? "")
  const [allowedProviders, setAllowedProviders] = useState(
    (existing?.allowed_providers ?? []).join(" "),
  )
  const [allowedTools, setAllowedTools] = useState((existing?.allowed_tools ?? []).join(" "))
  const [formError, setFormError] = useState<string | null>(null)

  const save = useMutation({
    mutationFn: () =>
      api.upsertAiPolicy({
        scope,
        subject_id: scope === "project" ? undefined : subjectId,
        default_provider: defaultProvider.trim() || null,
        default_model: defaultModel.trim() || null,
        allowed_providers: splitList(allowedProviders),
        allowed_tools: splitList(allowedTools),
      }),
    onSuccess: onSaved,
  })

  const submit = () => {
    setFormError(null)
    if (scope === "role" && !subjectId) {
      setFormError("Choose the role this policy applies to.")
      return
    }
    save.mutate()
  }

  return (
    <Dialog
      open
      onClose={onClose}
      title={existing ? "Edit AI policy" : "New AI policy"}
      description={
        existing
          ? "Saving replaces every field, so an omitted allow-list returns to unrestricted."
          : "A role policy narrows the project policy's allow-list — it can only restrict further, never widen it."
      }
      footer={
        <>
          <Button variant="outline" onClick={onClose} disabled={save.isPending}>
            Cancel
          </Button>
          <Button variant="gradient" onClick={submit} disabled={save.isPending}>
            {save.isPending ? "Saving…" : "Save policy"}
          </Button>
        </>
      }
    >
      <div className="space-y-4">
        <div className="space-y-1.5">
          <Label htmlFor="ai-policy-scope">Applies to</Label>
          <Select
            id="ai-policy-scope"
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
              Scope and subject identify a policy. Remove this one and create another to
              change them.
            </p>
          )}
        </div>

        {scope === "role" && (
          <div className="space-y-1.5">
            <Label htmlFor="ai-policy-role">Role</Label>
            <Select
              id="ai-policy-role"
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
            <Label htmlFor="ai-policy-default-provider">Default provider</Label>
            <Input
              id="ai-policy-default-provider"
              placeholder="anthropic"
              value={defaultProvider}
              onChange={(event) => setDefaultProvider(event.target.value)}
            />
          </div>
          <div className="space-y-1.5">
            <Label htmlFor="ai-policy-default-model">Default model</Label>
            <Input
              id="ai-policy-default-model"
              placeholder="anthropic:claude-sonnet-5"
              value={defaultModel}
              onChange={(event) => setDefaultModel(event.target.value)}
            />
          </div>
        </div>

        <div className="space-y-1.5">
          <Label htmlFor="ai-policy-providers">Allowed providers</Label>
          <Input
            id="ai-policy-providers"
            placeholder="anthropic googlegenai"
            value={allowedProviders}
            onChange={(event) => setAllowedProviders(event.target.value)}
          />
          <p className="text-[11px] text-(--color-text-subtle)">
            Space- or comma-separated. Empty means unrestricted, not "none allowed."
          </p>
        </div>

        <div className="space-y-1.5">
          <Label htmlFor="ai-policy-tools">Allowed tools</Label>
          <Input
            id="ai-policy-tools"
            placeholder="read write browser"
            value={allowedTools}
            onChange={(event) => setAllowedTools(event.target.value)}
          />
          <p className="text-[11px] text-(--color-text-subtle)">
            Space- or comma-separated. Empty means unrestricted, not "none allowed."
          </p>
        </div>

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
