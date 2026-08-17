import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"
import { Check, Minus, Pencil, Plus, Trash2 } from "lucide-react"
import { useState } from "react"

import { api, type PermissionGrant, type SubRole } from "@/shared/api/client"
import { PageFrame } from "@/shared/components/page-frame"
import { PRIMARY_ROLE_LABELS } from "@/shared/constants/member"
import {
  PERMISSION,
  constraintLabel,
  mayRequest,
  permissionLabel,
} from "@/shared/lib/authorization"
import { useAuthStore } from "@/shared/stores/auth"
import { Badge } from "@/shared/ui/badge"
import { Button } from "@/shared/ui/button"
import { Card, CardHeader, CardList, CardTitle } from "@/shared/ui/card"
import { ConfirmDialog, Dialog } from "@/shared/ui/dialog"
import { EmptyState, ErrorState } from "@/shared/ui/empty-state"
import { Input } from "@/shared/ui/input"
import { Label } from "@/shared/ui/label"
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

export function RolesPage() {
  const authorization = useAuthStore((state) => state.authorization)
  const can = useAuthStore((state) => state.can)
  const qc = useQueryClient()
  const { data = [], isLoading } = useQuery({
    queryKey: ["sub-roles"],
    queryFn: () => api.subRoles(),
  })
  const [editor, setEditor] = useState<SubRole | "new" | null>(null)
  const [pendingDelete, setPendingDelete] = useState<SubRole | null>(null)

  const remove = useMutation({
    mutationFn: (id: string) => api.deleteSubRole(id),
    onSuccess: () => {
      setPendingDelete(null)
      void qc.invalidateQueries({ queryKey: ["sub-roles"] })
      void qc.invalidateQueries({ queryKey: ["members"] })
    },
  })

  const canManage = mayRequest(can(PERMISSION.TAXONOMY_DEFINITION_MANAGE))
  const fixedRoles = authorization?.fixed_roles ?? []
  const permissions = authorization?.permission_metadata ?? []

  return (
    <PageFrame
      title="Roles"
      subtitle="Primary roles gate permissions. Sub-roles describe job functions (dev, ba, tester)."
      action={
        canManage ? (
          <Button variant="gradient" onClick={() => setEditor("new")}>
            <Plus className="size-3.5" />
            New sub-role
          </Button>
        ) : undefined
      }
    >
      {!canManage && (
        <p className="mb-4 rounded-lg border border-(--border-soft) bg-(--bg-key) px-3 py-2 text-sm text-(--color-text-muted)">
          Fixed role bundles and sub-roles are read-only for your account.
        </p>
      )}

      <div className="mb-6 grid gap-3 sm:grid-cols-2 md:grid-cols-3">
        {fixedRoles.map((item) => (
          <Card key={item.role} className="p-4">
            <div className="flex items-center justify-between gap-2">
              <div className="text-sm font-semibold">
                {PRIMARY_ROLE_LABELS[item.role]}
              </div>
              {authorization?.current_role === item.role && <Badge tone="accent">Current</Badge>}
            </div>
            <p className="mt-1 text-xs leading-relaxed text-(--color-text-muted)">
              Server-defined fixed bundle with {item.grants.length} permission grants.
            </p>
          </Card>
        ))}
      </div>

      <TableWrap className="mb-6">
        <Table>
          <TableHead>
            <tr>
              <TableTh>Permission boundary</TableTh>
              {fixedRoles.map((role) => (
                <TableTh key={role.role} className="text-center">
                  {PRIMARY_ROLE_LABELS[role.role]}
                </TableTh>
              ))}
            </tr>
          </TableHead>
          <TableBody>
            {permissions.map(({ key }) => (
              <TableRow key={key}>
                <TableTd>
                  <div className="font-medium">{permissionLabel(key)}</div>
                  <code className="text-[0.65rem] text-(--color-text-subtle)">{key}</code>
                </TableTd>
                {fixedRoles.map((role) => {
                  const grant = role.grants.find((item) => item.permission === key)
                  return (
                    <TableTd key={role.role} className="text-center">
                      <span className="sr-only">{grant ? "Granted" : "Not granted"}</span>
                      {grant ? (
                        <GrantCell grant={grant} />
                      ) : (
                        <Minus className="mx-auto size-4 text-(--color-text-subtle)" aria-hidden />
                      )}
                    </TableTd>
                  )
                })}
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </TableWrap>

      <Card>
        <CardHeader className="gap-3">
          <div>
            <CardTitle>Sub-roles</CardTitle>
            <p className="mt-1 text-xs text-(--color-text-muted)">
              Job functions assigned by project administrators.
            </p>
          </div>
        </CardHeader>

        {isLoading ? (
          <SkeletonRows rows={3} />
        ) : data.length === 0 ? (
          <div className="p-4">
            <EmptyState
              title="No sub-roles"
              description="Admins can define project-specific roles like developer, BA, or tester."
              className="border-0 bg-transparent py-8"
            />
          </div>
        ) : (
          <CardList>
            {data.map((role) => (
              <div key={role.id} className="flex items-center gap-3 px-4 py-3">
                <span
                  className="size-2.5 shrink-0 rounded-full"
                  style={{ background: role.color ?? "var(--color-accent)" }}
                />
                <div className="min-w-0 flex-1">
                  <div className="text-sm font-medium">{role.name}</div>
                  <div className="font-mono text-[0.7rem] text-(--color-text-subtle)">
                    {role.slug}
                  </div>
                </div>
                {canManage && (
                  <div className="flex items-center gap-1">
                    <Button
                      size="sm"
                      variant="ghost"
                      aria-label={`Edit ${role.name}`}
                      onClick={() => setEditor(role)}
                    >
                      <Pencil className="size-3.5" />
                    </Button>
                    <Button
                      size="sm"
                      variant="ghost"
                      aria-label={`Delete ${role.name}`}
                      onClick={() => setPendingDelete(role)}
                    >
                      <Trash2 className="size-3.5" />
                    </Button>
                  </div>
                )}
              </div>
            ))}
          </CardList>
        )}
      </Card>
      {editor && (
        <RoleDialog
          role={editor}
          onClose={() => setEditor(null)}
          onSaved={() => {
            setEditor(null)
            void qc.invalidateQueries({ queryKey: ["sub-roles"] })
          }}
        />
      )}
      {remove.error && (
        <ErrorState
          className="mt-4"
          message={remove.error instanceof Error ? remove.error.message : "Delete failed"}
        />
      )}
      <ConfirmDialog
        open={pendingDelete !== null}
        title={`Delete ${pendingDelete?.name ?? "sub-role"}?`}
        description="This sub-role will be removed from every member. Primary system permissions are unchanged."
        confirmLabel="Delete sub-role"
        busy={remove.isPending}
        onClose={() => setPendingDelete(null)}
        onConfirm={() => pendingDelete && remove.mutate(pendingDelete.id)}
      />
    </PageFrame>
  )
}

function GrantCell({ grant }: { grant: PermissionGrant }) {
  return (
    <span className="inline-flex max-w-40 flex-col items-center gap-1">
      <Check className="size-4 text-(--color-success)" aria-hidden />
      <span className="text-[0.62rem] leading-tight text-(--color-text-subtle)">
        {constraintLabel(grant.constraints)}
      </span>
      {grant.response_projection && (
        <Badge tone="neutral" className="text-[0.58rem]">
          {grant.response_projection.replaceAll("_", " ")}
        </Badge>
      )}
    </span>
  )
}

function RoleDialog({
  role,
  onClose,
  onSaved,
}: {
  role: SubRole | "new"
  onClose: () => void
  onSaved: () => void
}) {
  const existing = role === "new" ? null : role
  const [slug, setSlug] = useState(existing?.slug ?? "")
  const [name, setName] = useState(existing?.name ?? "")
  const [description, setDescription] = useState(existing?.description ?? "")
  const [color, setColor] = useState(existing?.color ?? "#667eea")
  const [error, setError] = useState<string | null>(null)

  const save = useMutation({
    mutationFn: () =>
      existing
        ? api.updateSubRole(existing.id, { name, description, color })
        : api.createSubRole({ slug, name, description, color }),
    onSuccess: onSaved,
    onError: (e) => setError(e instanceof Error ? e.message : "Save failed"),
  })

  return (
    <Dialog
      open
      title={existing ? "Edit sub-role" : "Create sub-role"}
      description="Sub-roles describe a member's project function. System permissions still come from the primary role."
      onClose={onClose}
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            Cancel
          </Button>
          <Button
            variant="gradient"
            disabled={!slug.trim() || !name.trim() || save.isPending}
            onClick={() => save.mutate()}
          >
            {existing ? "Save changes" : "Create sub-role"}
          </Button>
        </>
      }
    >
      <div className="space-y-4">
        <Field label="Name">
          <Input
            value={name}
            placeholder="Developer"
            autoFocus
            onChange={(e) => setName(e.target.value)}
          />
        </Field>
        <Field label="Slug">
          <Input
            value={slug}
            placeholder="dev"
            disabled={Boolean(existing)}
            onChange={(e) => setSlug(e.target.value)}
          />
        </Field>
        <Field label="Description">
          <Input
            value={description}
            placeholder="Builds and maintains project software"
            onChange={(e) => setDescription(e.target.value)}
          />
        </Field>
        <Field label="Color">
          <div className="flex items-center gap-2">
            <input
              type="color"
              value={color}
              aria-label="Sub-role color"
              className="size-9 rounded-md border border-(--color-border) bg-transparent p-1"
              onChange={(e) => setColor(e.target.value)}
            />
            <Input value={color} onChange={(e) => setColor(e.target.value)} />
          </div>
        </Field>
        {error && <ErrorState message={error} />}
      </div>
    </Dialog>
  )
}

function Field({
  label,
  children,
}: {
  label: string
  children: React.ReactNode
}) {
  return (
    <div className="space-y-1.5">
      <Label>{label}</Label>
      {children}
    </div>
  )
}
