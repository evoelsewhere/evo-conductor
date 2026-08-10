import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"
import {
  Check,
  Copy,
  Plus,
  RotateCcw,
  Search,
  SlidersHorizontal,
  UserCheck,
  UserX,
  X,
} from "lucide-react"
import { useMemo, useState } from "react"

import { MemberInstallationsPanel } from "@/features/members/components/member-installations-panel"
import {
  api,
  type PrimaryRole,
  type User,
  type UserStatus,
} from "@/shared/api/client"
import { PageFrame } from "@/shared/components/page-frame"
import { useAuthStore } from "@/shared/stores/auth"
import { Badge, StatusDot } from "@/shared/ui/badge"
import { BadgeList } from "@/shared/ui/badge-list"
import { Button } from "@/shared/ui/button"
import { EmptyState, ErrorState } from "@/shared/ui/empty-state"
import { ConfirmDialog, Dialog } from "@/shared/ui/dialog"
import { Input } from "@/shared/ui/input"
import { Label } from "@/shared/ui/label"
import {
  MultiSelect,
  type MultiSelectOption,
} from "@/shared/ui/multi-select"
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

const statusFilters = [
  { value: "", label: "All" },
  { value: "pending", label: "Pending" },
  { value: "invited", label: "Invited" },
  { value: "active", label: "Active" },
  { value: "disabled", label: "Disabled" },
] as const

const roleOptions = [
  { value: "admin", label: "Admin" },
  { value: "contribute", label: "Contribute" },
  { value: "user", label: "User" },
] as const

function statusTone(status: UserStatus): "success" | "warning" | "danger" | "accent" | "neutral" {
  switch (status) {
    case "active":
      return "success"
    case "pending":
      return "warning"
    case "invited":
      return "accent"
    case "disabled":
      return "danger"
  }
}

export function MembersPage() {
  const actor = useAuthStore((s) => s.user)
  const isAdmin = actor?.primary_role === "admin"
  const qc = useQueryClient()

  const [q, setQ] = useState("")
  const [status, setStatus] = useState<UserStatus | "">("")
  const [role, setRole] = useState<PrimaryRole | "">("")
  const [tag, setTag] = useState("")
  const [filtersOpen, setFiltersOpen] = useState(false)
  const [page, setPage] = useState(1)
  const limit = 50

  const [showAdd, setShowAdd] = useState(false)
  const [editUser, setEditUser] = useState<User | null>(null)
  const [tempPassword, setTempPassword] = useState<string | null>(null)
  const [confirmation, setConfirmation] = useState<{
    action: "disable" | "reset"
    member: User
  } | null>(null)

  const { data: tags = [] } = useQuery({
    queryKey: ["tags"],
    queryFn: () => api.tags(),
    enabled:
      actor?.primary_role === "admin" || actor?.primary_role === "contribute",
  })
  const { data: subRoles = [] } = useQuery({
    queryKey: ["sub-roles"],
    queryFn: () => api.subRoles(),
    enabled: isAdmin,
  })

  const { data, isLoading, error } = useQuery({
    queryKey: ["members", q, status, role, tag, page],
    queryFn: () =>
      api.members({
        q: q || undefined,
        status: status || undefined,
        role: role || undefined,
        tag: tag || undefined,
        page,
        limit,
      }),
  })

  const tagName = useMemo(() => {
    const map = new Map(tags.map((t) => [t.id, t.name]))
    tags.forEach((t) => map.set(t.slug, t.name))
    return (id: string) => map.get(id) ?? id
  }, [tags])

  const subRoleName = useMemo(() => {
    const map = new Map(subRoles.map((r) => [r.id, r.name]))
    return (id: string) => map.get(id) ?? id
  }, [subRoles])

  const tagOptions = useMemo(
    () => tags.map((t) => ({ value: t.id, label: t.name })),
    [tags],
  )
  const subRoleOptions = useMemo(
    () => subRoles.map((r) => ({ value: r.id, label: r.name })),
    [subRoles],
  )

  const approve = useMutation({
    mutationFn: (id: string) => api.approveMember(id),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: ["members"] })
      void qc.invalidateQueries({ queryKey: ["pending-count"] })
    },
  })
  const disable = useMutation({
    mutationFn: (id: string) => api.disableMember(id),
    onSuccess: () => {
      setConfirmation(null)
      void qc.invalidateQueries({ queryKey: ["members"] })
    },
  })
  const enable = useMutation({
    mutationFn: (id: string) => api.enableMember(id),
    onSuccess: () => void qc.invalidateQueries({ queryKey: ["members"] }),
  })
  const resetPw = useMutation({
    mutationFn: (id: string) => api.resetMemberPassword(id),
    onSuccess: (res) => {
      setConfirmation(null)
      setTempPassword(res.temporary_password)
    },
  })

  const items = data?.items ?? []
  const total = data?.total ?? 0
  const totalPages = Math.max(1, Math.ceil(total / limit))
  const activeFilterCount =
    Number(Boolean(status)) + Number(Boolean(role)) + Number(Boolean(tag))
  const actionError = [
    approve.error,
    disable.error,
    enable.error,
    resetPw.error,
  ].find((value): value is Error => value instanceof Error)

  return (
    <PageFrame
      title="Members"
      subtitle="Manage invitations, SSO approvals, account status, and project access."
      action={
        isAdmin ? (
          <Button variant="gradient" onClick={() => setShowAdd(true)}>
            <Plus className="size-3.5" />
            Add member
          </Button>
        ) : undefined
      }
    >
      {tempPassword && (
        <TempPasswordBanner
          password={tempPassword}
          onDismiss={() => setTempPassword(null)}
        />
      )}

      <div className="mb-4 rounded-xl border border-(--border-card) bg-(--bg-card)">
        <div className="flex flex-col gap-2 p-3 sm:flex-row sm:items-center">
          <div className="relative min-w-0 flex-1 sm:max-w-md">
            <Search className="pointer-events-none absolute top-1/2 left-3 size-3.5 -translate-y-1/2 text-(--color-text-subtle)" />
            <Input
              className="pl-9"
              placeholder="Search members by name or email"
              value={q}
              onChange={(e) => {
                setQ(e.target.value)
                setPage(1)
              }}
            />
          </div>
          <Button
            variant={filtersOpen || activeFilterCount > 0 ? "secondary" : "outline"}
            onClick={() => setFiltersOpen((open) => !open)}
          >
            <SlidersHorizontal className="size-3.5" />
            Filters
            {activeFilterCount > 0 && (
              <span className="rounded-full bg-(--color-accent-soft) px-1.5 text-[0.65rem] text-(--color-accent)">
                {activeFilterCount}
              </span>
            )}
          </Button>
        </div>

        {filtersOpen && (
          <div className="border-t border-(--border-soft) p-4">
            <div className="grid gap-4 sm:grid-cols-3">
              {isAdmin && (
                <FilterField label="Account status">
                  <Select
                    value={status || "__any__"}
                    onValueChange={(v) => {
                      setStatus(v === "__any__" ? "" : (v as UserStatus))
                      setPage(1)
                    }}
                    options={[
                      { value: "__any__", label: "Any status" },
                      ...statusFilters
                        .filter((item) => item.value)
                        .map((item) => ({
                          value: item.value,
                          label: item.label,
                        })),
                    ]}
                  />
                </FilterField>
              )}
              <FilterField label="System role">
                <Select
                  value={role || "__any__"}
                  onValueChange={(v) => {
                    setRole(v === "__any__" ? "" : (v as PrimaryRole))
                    setPage(1)
                  }}
                  options={[
                    { value: "__any__", label: "Any role" },
                    ...roleOptions,
                  ]}
                />
              </FilterField>
              <FilterField label="Tag">
                <Select
                  value={tag || "__any__"}
                  onValueChange={(v) => {
                    setTag(v === "__any__" ? "" : v)
                    setPage(1)
                  }}
                  options={[
                    { value: "__any__", label: "Any tag" },
                    ...tagOptions,
                  ]}
                />
              </FilterField>
            </div>
            {activeFilterCount > 0 && (
              <Button
                size="sm"
                variant="ghost"
                className="mt-3"
                onClick={() => {
                  setStatus("")
                  setRole("")
                  setTag("")
                  setPage(1)
                }}
              >
                <X className="size-3.5" />
                Clear filters
              </Button>
            )}
          </div>
        )}
      </div>

      {error && (
        <ErrorState
          className="mb-4"
          message={error instanceof Error ? error.message : "Failed to load"}
        />
      )}

      {actionError && (
        <ErrorState
          className="mb-4"
          message={actionError.message}
        />
      )}

      {isLoading ? (
        <TableWrap>
          <SkeletonRows rows={6} />
        </TableWrap>
      ) : items.length === 0 ? (
        <EmptyState
          title="No members match"
          description="Adjust filters, or add a member with a temporary password."
        />
      ) : (
        <>
          <TableWrap>
            <Table>
              <TableHead>
                <tr>
                  <TableTh>Member</TableTh>
                  <TableTh>Role</TableTh>
                  <TableTh>Tags</TableTh>
                  <TableTh>Status</TableTh>
                  {isAdmin && <TableTh />}
                </tr>
              </TableHead>
              <TableBody>
                {items.map((m) => (
                  <TableRow key={m.id}>
                    <TableTd>
                      <div className="font-medium">{m.display_name}</div>
                      <div className="text-xs text-(--color-text-subtle)">
                        {m.email}
                      </div>
                      {isAdmin && m.sub_role_ids.length > 0 && (
                        <BadgeList
                          className="mt-1"
                          max={2}
                          items={m.sub_role_ids.map(subRoleName)}
                        />
                      )}
                    </TableTd>
                    <TableTd>
                      <Badge tone="accent" className="capitalize">
                        {m.primary_role}
                      </Badge>
                    </TableTd>
                    <TableTd>
                      <BadgeList
                        className="max-w-48"
                        max={2}
                        items={m.tag_ids.map(tagName)}
                      />
                    </TableTd>
                    <TableTd>
                      <span className="inline-flex items-center gap-1.5 capitalize text-(--color-text-muted)">
                        <StatusDot tone={statusTone(m.status)} />
                        {m.status}
                      </span>
                    </TableTd>
                    {isAdmin && (
                      <TableTd>
                        <div className="flex justify-end gap-1">
                          {(m.status === "pending" || m.status === "invited") && (
                            <Button
                              size="sm"
                              variant="ghost"
                              title="Approve"
                              onClick={() => approve.mutate(m.id)}
                            >
                              <UserCheck className="size-3.5" />
                            </Button>
                          )}
                          <Button
                            size="sm"
                            variant="ghost"
                            title="Edit"
                            onClick={() => setEditUser(m)}
                          >
                            Edit
                          </Button>
                          {m.status === "disabled" ? (
                            <Button
                              size="sm"
                              variant="ghost"
                              onClick={() => enable.mutate(m.id)}
                            >
                              Enable
                            </Button>
                          ) : m.id !== actor?.id ? (
                            <Button
                              size="sm"
                              variant="ghost"
                              title="Disable"
                              onClick={() =>
                                setConfirmation({ action: "disable", member: m })
                              }
                            >
                              <UserX className="size-3.5" />
                            </Button>
                          ) : null}
                          <Button
                            size="sm"
                            variant="ghost"
                            title="Reset password"
                            onClick={() =>
                              setConfirmation({ action: "reset", member: m })
                            }
                          >
                            <RotateCcw className="size-3.5" />
                          </Button>
                        </div>
                      </TableTd>
                    )}
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </TableWrap>

          <div className="mt-3 flex items-center justify-between text-xs text-(--color-text-muted)">
            <span>
              {total} member{total === 1 ? "" : "s"} · page {page}/{totalPages}
            </span>
            <div className="flex gap-1">
              <Button
                size="sm"
                variant="outline"
                disabled={page <= 1}
                onClick={() => setPage((p) => p - 1)}
              >
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
        </>
      )}

      {showAdd && (
        <MemberDialog
          title="Add member"
          tags={tagOptions}
          subRoles={subRoleOptions}
          onClose={() => setShowAdd(false)}
          onCreated={(pw) => {
            setShowAdd(false)
            setTempPassword(pw)
            void qc.invalidateQueries({ queryKey: ["members"] })
          }}
        />
      )}

      {editUser && (
        <EditMemberDialog
          user={editUser}
          tags={tagOptions}
          subRoles={subRoleOptions}
          onClose={() => setEditUser(null)}
          onSaved={() => {
            setEditUser(null)
            void qc.invalidateQueries({ queryKey: ["members"] })
          }}
        />
      )}

      <ConfirmDialog
        open={confirmation !== null}
        title={
          confirmation?.action === "disable"
            ? `Disable ${confirmation.member.display_name}?`
            : `Reset ${confirmation?.member.display_name ?? "member"}'s password?`
        }
        description={
          confirmation?.action === "disable"
            ? "Their browser sessions and EvoFlux connection secrets will stop working immediately."
            : "Existing browser sessions will be revoked. A new temporary password will be shown once."
        }
        confirmLabel={confirmation?.action === "disable" ? "Disable member" : "Reset password"}
        busy={disable.isPending || resetPw.isPending}
        onClose={() => setConfirmation(null)}
        onConfirm={() => {
          if (!confirmation) return
          if (confirmation.action === "disable") disable.mutate(confirmation.member.id)
          else resetPw.mutate(confirmation.member.id)
        }}
      />
    </PageFrame>
  )
}

function TempPasswordBanner({
  password,
  onDismiss,
}: {
  password: string
  onDismiss: () => void
}) {
  return (
    <div className="mb-4 rounded-xl border border-(--accent-blue)/30 bg-(--accent-blue)/8 px-4 py-3">
      <div className="mb-1 text-xs font-medium text-(--accent-blue-text)">
        Temporary password — copy now, it won’t be shown again
      </div>
      <div className="flex flex-col gap-2 sm:flex-row sm:items-center">
        <code className="min-w-0 flex-1 rounded-md bg-(--bg-page) px-2 py-1.5 font-mono text-sm">
          {password}
        </code>
        <Button
          variant="outline"
          size="sm"
          onClick={() => void navigator.clipboard.writeText(password)}
        >
          <Copy className="size-3.5" />
          Copy
        </Button>
        <Button variant="ghost" size="sm" onClick={onDismiss}>
          <Check className="size-3.5" />
          Done
        </Button>
      </div>
    </div>
  )
}

function MemberDialog({
  title,
  tags,
  subRoles,
  onClose,
  onCreated,
}: {
  title: string
  tags: MultiSelectOption[]
  subRoles: MultiSelectOption[]
  onClose: () => void
  onCreated: (tempPassword: string) => void
}) {
  const [email, setEmail] = useState("")
  const [displayName, setDisplayName] = useState("")
  const [primaryRole, setPrimaryRole] = useState<PrimaryRole>("user")
  const [subRoleIds, setSubRoleIds] = useState<string[]>([])
  const [tagIds, setTagIds] = useState<string[]>([])
  const [error, setError] = useState<string | null>(null)

  const create = useMutation({
    mutationFn: () =>
      api.createMember({
        email,
        display_name: displayName,
        primary_role: primaryRole,
        sub_role_ids: subRoleIds,
        tag_ids: tagIds,
      }),
    onSuccess: (res) => onCreated(res.temporary_password),
    onError: (e) => setError(e instanceof Error ? e.message : "Failed"),
  })

  return (
    <Dialog open title={title} onClose={onClose}>
      <div className="space-y-3">
        <Field label="Email">
          <Input value={email} onChange={(e) => setEmail(e.target.value)} type="email" />
        </Field>
        <Field label="Display name">
          <Input
            value={displayName}
            onChange={(e) => setDisplayName(e.target.value)}
          />
        </Field>
        <Field label="Primary role">
          <Select
            value={primaryRole}
            onValueChange={(v) => setPrimaryRole(v as PrimaryRole)}
            options={[...roleOptions]}
          />
        </Field>
        <Field label="Sub-roles" hint="Job function within the project.">
          <MultiSelect
            options={subRoles}
            value={subRoleIds}
            onChange={setSubRoleIds}
            placeholder="Search sub-roles…"
            emptyLabel="No sub-roles defined yet. Create them under Roles."
          />
        </Field>
        <Field label="Tags" hint="Squad, stream, or any grouping you filter by.">
          <MultiSelect
            options={tags}
            value={tagIds}
            onChange={setTagIds}
            placeholder="Search tags…"
            emptyLabel="No tags defined yet. Create them under Tags."
          />
        </Field>
        {error && <ErrorState message={error} />}
        <div className="flex justify-end gap-2 pt-2">
          <Button variant="ghost" onClick={onClose}>
            Cancel
          </Button>
          <Button
            variant="gradient"
            disabled={!email.includes("@") || !displayName.trim() || create.isPending}
            onClick={() => create.mutate()}
          >
            Create & show password
          </Button>
        </div>
      </div>
    </Dialog>
  )
}

function EditMemberDialog({
  user,
  tags,
  subRoles,
  onClose,
  onSaved,
}: {
  user: User
  tags: MultiSelectOption[]
  subRoles: MultiSelectOption[]
  onClose: () => void
  onSaved: () => void
}) {
  const actorId = useAuthStore((state) => state.user?.id)
  const editingSelf = actorId === user.id
  const [displayName, setDisplayName] = useState(user.display_name)
  const [primaryRole, setPrimaryRole] = useState<PrimaryRole>(user.primary_role)
  const [subRoleIds, setSubRoleIds] = useState(user.sub_role_ids)
  const [tagIds, setTagIds] = useState(user.tag_ids)
  const [error, setError] = useState<string | null>(null)

  const save = useMutation({
    mutationFn: () =>
      api.updateMember(user.id, {
        display_name: displayName,
        primary_role: primaryRole,
        sub_role_ids: subRoleIds,
        tag_ids: tagIds,
      }),
    onSuccess: onSaved,
    onError: (e) => setError(e instanceof Error ? e.message : "Failed"),
  })

  return (
    <Dialog open title={`Edit ${user.email}`} onClose={onClose}>
      <div className="space-y-3">
        <Field label="Display name">
          <Input
            value={displayName}
            onChange={(e) => setDisplayName(e.target.value)}
          />
        </Field>
        <Field
          label="Primary role"
          hint={editingSelf ? "Ask another admin to change your primary role." : undefined}
        >
          <Select
            value={primaryRole}
            onValueChange={(v) => setPrimaryRole(v as PrimaryRole)}
            options={[...roleOptions]}
            disabled={editingSelf}
          />
        </Field>
        <Field label="Sub-roles" hint="Job function within the project.">
          <MultiSelect
            options={subRoles}
            value={subRoleIds}
            onChange={setSubRoleIds}
            placeholder="Search sub-roles…"
            emptyLabel="No sub-roles defined yet. Create them under Roles."
          />
        </Field>
        <Field label="Tags" hint="Squad, stream, or any grouping you filter by.">
          <MultiSelect
            options={tags}
            value={tagIds}
            onChange={setTagIds}
            placeholder="Search tags…"
            emptyLabel="No tags defined yet. Create them under Tags."
          />
        </Field>
        <MemberInstallationsPanel userId={user.id} />
        {error && <ErrorState message={error} />}
        <div className="flex justify-end gap-2 pt-2">
          <Button variant="ghost" onClick={onClose}>
            Cancel
          </Button>
          <Button
            variant="gradient"
            disabled={!displayName.trim() || save.isPending}
            onClick={() => save.mutate()}
          >
            Save
          </Button>
        </div>
      </div>
    </Dialog>
  )
}

function Field({
  label,
  hint,
  children,
}: {
  label: string
  hint?: string
  children: React.ReactNode
}) {
  return (
    <div className="space-y-1.5">
      <div>
        <Label>{label}</Label>
        {hint && (
          <p className="mt-0.5 text-xs text-(--color-text-subtle)">{hint}</p>
        )}
      </div>
      {children}
    </div>
  )
}

function FilterField({
  label,
  children,
}: {
  label: string
  children: React.ReactNode
}) {
  return (
    <div className="space-y-1.5">
      <div className="text-xs font-medium text-(--color-text-muted)">{label}</div>
      {children}
    </div>
  )
}
