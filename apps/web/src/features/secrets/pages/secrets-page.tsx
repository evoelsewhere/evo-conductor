import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"
import { Check, Copy, KeyRound, Plus, Trash2, X } from "lucide-react"
import { useState } from "react"

import {
  api,
  type ConnectionSecret,
  type SecretScope,
} from "@/shared/api/client"
import { PageFrame } from "@/shared/components/page-frame"
import { PERMISSION, mayRequest } from "@/shared/lib/authorization"
import { useAuthStore } from "@/shared/stores/auth"
import {
  SECRET_SCOPE,
  SECRET_SCOPE_OPTIONS,
} from "@/shared/constants/secret"
import { Badge } from "@/shared/ui/badge"
import { BadgeList } from "@/shared/ui/badge-list"
import { Button } from "@/shared/ui/button"
import { ConfirmDialog, Dialog } from "@/shared/ui/dialog"
import { EmptyState, ErrorState } from "@/shared/ui/empty-state"
import { Input } from "@/shared/ui/input"
import { Label } from "@/shared/ui/label"
import { Select } from "@/shared/ui/select"
import { SkeletonRows } from "@/shared/ui/skeleton"
import {
  Table,
  TableBody,
  TableHead,
  TableRow,
  TableTd,
  TableTh,
  TableWrap,
} from "@/shared/ui/table"

const expirationOptions = [
  { value: "30", label: "30 days" },
  { value: "90", label: "90 days" },
  { value: "365", label: "1 year" },
  { value: "never", label: "No expiration" },
] as const

export function SecretsPage() {
  const qc = useQueryClient()
  const userId = useAuthStore((state) => state.user?.id)
  const can = useAuthStore((state) => state.can)
  const canIssue = mayRequest(
    can(PERMISSION.CONNECTION_TOKEN_ISSUE_SELF, { ownerId: userId }),
  )
  const canRevoke = mayRequest(
    can(PERMISSION.CONNECTION_TOKEN_REVOKE_SELF, { ownerId: userId }),
  )
  const { data = [], isLoading, error } = useQuery({
    queryKey: ["secrets"],
    queryFn: () => api.secrets(),
  })
  const [showCreate, setShowCreate] = useState(false)
  const [createdToken, setCreatedToken] = useState<string | null>(null)
  const [pendingRevoke, setPendingRevoke] = useState<ConnectionSecret | null>(null)

  const revoke = useMutation({
    mutationFn: (id: string) => api.revokeSecret(id),
    onSuccess: () => {
      setPendingRevoke(null)
      void qc.invalidateQueries({ queryKey: ["secrets"] })
    },
  })

  return (
    <PageFrame
      title="Connection secrets"
      subtitle="Issue least-privilege, expiring tokens for EvoFlux clients."
      action={
        canIssue ? <Button variant="gradient" onClick={() => setShowCreate(true)}>
          <Plus className="size-3.5" />
          New token
        </Button> : undefined
      }
    >
      {createdToken && (
        <div className="mb-4 rounded-xl border border-(--accent-blue)/30 bg-(--accent-blue)/8 px-4 py-3">
          <div className="flex items-start gap-3">
            <div className="min-w-0 flex-1">
              <div className="mb-1 text-xs font-medium text-(--accent-blue-text)">
                Copy this token now — it won’t be shown again
              </div>
              <code className="block overflow-x-auto rounded-md bg-(--bg-page) px-2 py-1.5 font-mono text-xs">
                {createdToken}
              </code>
            </div>
            <Button
              variant="ghost"
              size="icon"
              aria-label="Dismiss token"
              onClick={() => setCreatedToken(null)}
            >
              <X className="size-3.5" />
            </Button>
          </div>
          <Button
            variant="outline"
            size="sm"
            className="mt-2"
            onClick={() => void navigator.clipboard.writeText(createdToken)}
          >
            <Copy className="size-3.5" />
            Copy token
          </Button>
        </div>
      )}

      {(error || revoke.error) && (
        <ErrorState
          className="mb-4"
          message={
            error instanceof Error
              ? error.message
              : revoke.error instanceof Error
                ? revoke.error.message
                : "Secret action failed"
          }
        />
      )}

      {isLoading ? (
        <TableWrap>
          <SkeletonRows rows={4} />
        </TableWrap>
      ) : data.length === 0 ? (
        <EmptyState
          icon={KeyRound}
          title="No secrets yet"
          description="Create a scoped connection token and paste it into EvoFlux."
          action={canIssue ? (
            <Button variant="outline" onClick={() => setShowCreate(true)}>
              Create first token
            </Button>
          ) : undefined}
        />
      ) : (
        <TableWrap>
          <Table>
            <TableHead>
              <tr>
                <TableTh>Name</TableTh>
                <TableTh>Prefix</TableTh>
                <TableTh>Scopes</TableTh>
                <TableTh>Last used</TableTh>
                <TableTh>Status</TableTh>
                <TableTh />
              </tr>
            </TableHead>
            <TableBody>
              {data.map((secret) => {
                const expired =
                  secret.expires_at !== null &&
                  new Date(secret.expires_at).getTime() <= Date.now()
                return (
                  <TableRow key={secret.id}>
                    <TableTd className="font-medium">{secret.name}</TableTd>
                    <TableTd className="font-mono text-xs text-(--color-text-muted)">
                      evc_{secret.prefix}_…
                    </TableTd>
                    <TableTd>
                      <BadgeList
                        className="max-w-xs"
                        max={3}
                        items={secret.scopes}
                      />
                    </TableTd>
                    <TableTd className="text-xs text-(--color-text-muted)">
                      {secret.last_used_at
                        ? new Date(secret.last_used_at).toLocaleString()
                        : "Never"}
                    </TableTd>
                    <TableTd className="text-xs">
                      {secret.revoked_at ? (
                        <Badge tone="danger">Revoked</Badge>
                      ) : expired ? (
                        <Badge tone="warning">Expired</Badge>
                      ) : (
                        <Badge tone="success">Active</Badge>
                      )}
                    </TableTd>
                    <TableTd className="text-right">
                      {canRevoke && !secret.revoked_at && !expired && (
                        <Button
                          variant="ghost"
                          size="sm"
                          aria-label={`Revoke ${secret.name}`}
                          onClick={() => setPendingRevoke(secret)}
                        >
                          <Trash2 className="size-3.5" />
                        </Button>
                      )}
                    </TableTd>
                  </TableRow>
                )
              })}
            </TableBody>
          </Table>
        </TableWrap>
      )}

      <CreateSecretDialog
        open={showCreate && canIssue}
        onClose={() => setShowCreate(false)}
        onCreated={(token) => {
          setShowCreate(false)
          setCreatedToken(token)
          void qc.invalidateQueries({ queryKey: ["secrets"] })
        }}
      />
      <ConfirmDialog
        open={pendingRevoke !== null}
        title={`Revoke ${pendingRevoke?.name ?? "secret"}?`}
        description="The associated EvoFlux client will lose access immediately and this token cannot be restored."
        confirmLabel="Revoke secret"
        busy={revoke.isPending}
        onClose={() => setPendingRevoke(null)}
        onConfirm={() => pendingRevoke && revoke.mutate(pendingRevoke.id)}
      />
    </PageFrame>
  )
}

function CreateSecretDialog({
  open,
  onClose,
  onCreated,
}: {
  open: boolean
  onClose: () => void
  onCreated: (token: string) => void
}) {
  const [name, setName] = useState("EvoFlux laptop")
  const [scopes, setScopes] = useState<SecretScope[]>([
    SECRET_SCOPE.SUBSCRIBE_RESOURCES,
  ])
  const [expiresIn, setExpiresIn] = useState("90")

  const create = useMutation({
    mutationFn: () => {
      const days = expiresIn === "never" ? null : Number(expiresIn)
      const expiresAt = days
        ? new Date(Date.now() + days * 86_400_000).toISOString()
        : undefined
      return api.createSecret({
        name: name.trim(),
        scopes,
        expires_at: expiresAt,
      })
    },
    onSuccess: (result) => onCreated(result.token),
  })

  function toggleScope(scope: SecretScope) {
    setScopes((current) =>
      current.includes(scope)
        ? current.filter((value) => value !== scope)
        : [...current, scope],
    )
  }

  return (
    <Dialog
      open={open}
      title="Create connection secret"
      description="Choose only the capabilities this EvoFlux client needs."
      onClose={onClose}
      footer={
        <>
          <Button variant="ghost" disabled={create.isPending} onClick={onClose}>
            Cancel
          </Button>
          <Button
            variant="gradient"
            disabled={!name.trim() || scopes.length === 0 || create.isPending}
            onClick={() => create.mutate()}
          >
            {create.isPending ? "Creating…" : "Create secret"}
          </Button>
        </>
      }
    >
      <div className="space-y-4">
        <div className="space-y-1.5">
          <Label htmlFor="secret-name">Name</Label>
          <Input
            id="secret-name"
            value={name}
            autoFocus
            onChange={(event) => setName(event.target.value)}
          />
        </div>
        <fieldset className="space-y-2">
          <legend className="text-[0.8rem] font-medium text-(--color-text-2)">
            Scopes
          </legend>
          {SECRET_SCOPE_OPTIONS.map((option) => {
            const checked = scopes.includes(option.value)
            return (
              <label
                key={option.value}
                className="flex cursor-pointer items-start gap-3 rounded-lg border border-(--color-border) p-3 transition-colors hover:bg-(--bg-key)/60"
              >
                <input
                  type="checkbox"
                  className="sr-only"
                  checked={checked}
                  onChange={() => toggleScope(option.value)}
                />
                <span
                  className={`mt-0.5 grid size-4 shrink-0 place-items-center rounded-sm border ${
                    checked
                      ? "border-(--color-accent) bg-(--color-accent) text-(--color-text-on-accent)"
                      : "border-(--color-border-strong)"
                  }`}
                >
                  {checked && <Check className="size-3" />}
                </span>
                <span className="min-w-0">
                  <span className="block text-sm font-medium">{option.label}</span>
                  <span className="mt-0.5 block text-xs text-(--color-text-subtle)">
                    {option.description}
                  </span>
                </span>
              </label>
            )
          })}
        </fieldset>
        <div className="space-y-1.5">
          <Label htmlFor="secret-expiration">Expires</Label>
          <Select
            id="secret-expiration"
            value={expiresIn}
            onValueChange={setExpiresIn}
            options={expirationOptions}
          />
        </div>
        {create.error && (
          <ErrorState
            message={
              create.error instanceof Error ? create.error.message : "Create failed"
            }
          />
        )}
      </div>
    </Dialog>
  )
}
