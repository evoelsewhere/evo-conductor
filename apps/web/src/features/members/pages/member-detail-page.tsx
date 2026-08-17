import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"
import { Link, useParams } from "@tanstack/react-router"
import {
  Activity,
  ArrowLeft,
  Bot,
  Check,
  Copy,
  KeyRound,
  Pencil,
  Plus,
  Radio,
  Wrench,
} from "lucide-react"
import { useMemo, useState } from "react"

import {
  DateRangeFilter,
  useUsageRange,
} from "@/features/members/components/date-range-filter"
import { MemberInstallationsPanel } from "@/features/members/components/member-installations-panel"
import { MemberActivityTable } from "@/features/members/components/member-activity-table"
import { MemberNav } from "@/features/members/components/member-nav"
import {
  formatTokens,
  ModelDonutChart,
  TokenTrendChart,
} from "@/features/members/components/usage-charts"
import {
  api,
  type PrimaryRole,
  type User,
} from "@/shared/api/client"
import { PageFrame } from "@/shared/components/page-frame"
import { StatCard, StatCardGrid, StatCardGridSkeleton } from "@/shared/components/stat-card"
import {
  MEMBER_QUERY_KEYS,
  MEMBER_STATUS_TONES,
  PRIMARY_ROLE_OPTIONS,
} from "@/shared/constants/member"
import { CONNECTION_SECRET_SCOPES } from "@/shared/constants/secret"
import {
  TELEMETRY_QUERY_KEYS,
  TELEMETRY_RECENT_ACTIVITY_LIMIT,
} from "@/shared/constants/telemetry"
import { useAuthStore } from "@/shared/stores/auth"
import {
  PERMISSION,
  bestAuthorizationDecision,
  mayRequest,
} from "@/shared/lib/authorization"
import { Badge, StatusDot } from "@/shared/ui/badge"
import { Button } from "@/shared/ui/button"
import { Card, CardContent, CardHeader, CardTitle } from "@/shared/ui/card"
import { Dialog } from "@/shared/ui/dialog"
import { EmptyState, ErrorState } from "@/shared/ui/empty-state"
import { Input } from "@/shared/ui/input"
import { Label } from "@/shared/ui/label"
import { MultiSelect } from "@/shared/ui/multi-select"
import { Select } from "@/shared/ui/select"

export function MemberDetailPage() {
  const { userId } = useParams({ strict: false }) as { userId: string }
  const can = useAuthStore((state) => state.can)
  const canManage = mayRequest(can(PERMISSION.MEMBER_MANAGE, { targetId: userId }))
  const canViewSecrets = mayRequest(
    bestAuthorizationDecision([
      can(PERMISSION.CONNECTION_TOKEN_READ_SELF, {
        targetId: userId,
        ownerId: userId,
      }),
      can(PERMISSION.CONNECTION_TOKEN_READ_ANY, { targetId: userId }),
    ]),
  )
  const canIssueToken = mayRequest(
    can(PERMISSION.CONNECTION_TOKEN_ISSUE_SELF, { ownerId: userId }),
  )
  const canRevokeToken = mayRequest(
    bestAuthorizationDecision([
      can(PERMISSION.CONNECTION_TOKEN_REVOKE_SELF, {
        targetId: userId,
        ownerId: userId,
      }),
      can(PERMISSION.CONNECTION_TOKEN_REVOKE_ANY, { targetId: userId }),
    ]),
  )
  const qc = useQueryClient()
  const dates = useUsageRange()
  const [editOpen, setEditOpen] = useState(false)
  const [tokenOpen, setTokenOpen] = useState(false)

  const member = useQuery({
    queryKey: MEMBER_QUERY_KEYS.detail(userId),
    queryFn: () => api.getMember(userId),
  })
  const usage = useQuery({
    queryKey: TELEMETRY_QUERY_KEYS.summary(userId, dates.range.from, dates.range.to),
    queryFn: () => api.memberUsageSummary(userId, dates.range),
  })
  const recent = useQuery({
    queryKey: TELEMETRY_QUERY_KEYS.activity(
      userId,
      dates.range.from,
      dates.range.to,
      TELEMETRY_RECENT_ACTIVITY_LIMIT,
    ),
    queryFn: () =>
      api.memberActivity(userId, {
        ...dates.range,
        limit: TELEMETRY_RECENT_ACTIVITY_LIMIT,
      }),
  })

  const refreshMember = () => {
    void qc.invalidateQueries({ queryKey: MEMBER_QUERY_KEYS.detail(userId) })
    void qc.invalidateQueries({ queryKey: MEMBER_QUERY_KEYS.list })
  }

  return (
    <PageFrame
      title={member.data?.display_name ?? "Member overview"}
      subtitle={member.data ? `${member.data.email} · ${member.data.primary_role}` : undefined}
      action={
        canManage && member.data ? (
          <Button variant="outline" onClick={() => setEditOpen(true)}>
            <Pencil className="size-3.5" />
            Edit member
          </Button>
        ) : undefined
      }
    >
      <Link
        to="/app/members"
        className="mb-3 inline-flex items-center gap-1 text-xs text-(--color-text-muted) hover:text-(--color-text)"
      >
        <ArrowLeft className="size-3.5" />
        All members
      </Link>
      <MemberNav userId={userId} />

      {member.error && <ErrorState className="mb-4" message={member.error.message} />}
      {usage.error && <ErrorState className="mb-4" message={usage.error.message} />}

      <div className="mb-4 flex flex-wrap items-center justify-between gap-3">
        {member.data && (
          <div className="flex items-center gap-2 text-sm">
            <Badge tone={MEMBER_STATUS_TONES[member.data.status]}>
              <StatusDot tone={MEMBER_STATUS_TONES[member.data.status]} />
              {member.data.status}
            </Badge>
            <Badge tone="accent" className="capitalize">{member.data.primary_role}</Badge>
          </div>
        )}
        <DateRangeFilter
          preset={dates.preset}
          onPresetChange={dates.setPreset}
          customFrom={dates.customFrom}
          onCustomFromChange={dates.setCustomFrom}
          customTo={dates.customTo}
          onCustomToChange={dates.setCustomTo}
        />
      </div>

      {usage.isLoading ? (
        <StatCardGridSkeleton count={4} />
      ) : (
        <StatCardGrid>
          <StatCard
            label="Total tokens"
            value={formatTokens(usage.data?.total_tokens ?? 0)}
            hint={`${formatTokens(usage.data?.tokens_in ?? 0)} in · ${formatTokens(usage.data?.tokens_out ?? 0)} out`}
            icon={Activity}
            tone="accent"
          />
          <StatCard
            label="Requests"
            value={usage.data?.total_requests ?? 0}
            hint={`${usage.data?.model_calls ?? 0} model calls`}
            icon={Radio}
          />
          <StatCard
            label="Tool calls"
            value={usage.data?.tool_calls ?? 0}
            hint={`${usage.data?.error_count ?? 0} errors across all events`}
            icon={Wrench}
            tone="success"
          />
          <StatCard
            label="Cache read"
            value={formatTokens(usage.data?.cache_read_tokens ?? 0)}
            hint={`${formatTokens(usage.data?.reasoning_tokens ?? 0)} reasoning tokens`}
            icon={Bot}
            tone="warning"
          />
        </StatCardGrid>
      )}

      <div className="mt-4 grid gap-4 lg:grid-cols-2">
        <TokenTrendChart daily={usage.data?.daily ?? []} />
        <ModelDonutChart models={usage.data?.models ?? []} />
      </div>

      <Card className="mt-4">
        <CardHeader>
          <CardTitle>Recent activity</CardTitle>
          <Link
            to="/app/members/$userId/activity"
            params={{ userId }}
            className="text-xs font-medium text-(--color-accent) hover:underline"
          >
            View all requests
          </Link>
        </CardHeader>
        <CardContent className="p-0">
          {recent.data?.items.length === 0 ? (
            <EmptyState
              title="No requests in this range"
              description="Usage appears after this member runs EvoFlux while Conductor sync is enabled."
              className="border-0 py-10"
            />
          ) : (
            <MemberActivityTable
              userId={userId}
              items={recent.data?.items ?? []}
              density="compact"
            />
          )}
        </CardContent>
      </Card>

      <div className="mt-4 grid gap-4 lg:grid-cols-2">
        <Card>
          <CardContent>
            <MemberInstallationsPanel userId={userId} />
          </CardContent>
        </Card>
        {canViewSecrets && (
          <MemberSecretsCard
            userId={userId}
            canCreate={canIssueToken}
            canRevoke={canRevokeToken}
            onCreate={() => setTokenOpen(true)}
          />
        )}
      </div>

      {editOpen && member.data && (
        <EditMemberDialog
          user={member.data}
          onClose={() => setEditOpen(false)}
          onSaved={() => {
            setEditOpen(false)
            refreshMember()
          }}
        />
      )}
      {tokenOpen && canIssueToken && (
        <CreateTokenDialog userId={userId} onClose={() => setTokenOpen(false)} />
      )}
    </PageFrame>
  )
}

function MemberSecretsCard({
  userId,
  canCreate,
  canRevoke,
  onCreate,
}: {
  userId: string
  canCreate: boolean
  canRevoke: boolean
  onCreate: () => void
}) {
  const qc = useQueryClient()
  const query = useQuery({ queryKey: MEMBER_QUERY_KEYS.secrets(userId), queryFn: () => api.memberSecrets(userId) })
  const revoke = useMutation({
    mutationFn: (secretId: string) => api.revokeMemberSecret(userId, secretId),
    onSuccess: () => void qc.invalidateQueries({ queryKey: MEMBER_QUERY_KEYS.secrets(userId) }),
  })
  const active = query.data?.filter((secret) => !secret.revoked_at) ?? []
  return (
    <Card>
      <CardHeader>
        <div><CardTitle>Connection tokens</CardTitle><p className="mt-0.5 text-xs text-(--color-text-muted)">Tokens this member can use to connect EvoFlux.</p></div>
        {canCreate && (
          <Button size="sm" variant="outline" onClick={onCreate}>
            <Plus className="size-3.5" />Create
          </Button>
        )}
      </CardHeader>
      <CardContent>
        {query.error && <ErrorState message={query.error.message} />}
        {active.length === 0 ? (
          <EmptyState
            icon={KeyRound}
            title="No active tokens"
            description={canCreate ? "Create a scoped token for your own EvoFlux client." : "This member has no active connection-token metadata."}
            className="border-0 py-7"
          />
        ) : (
          <div className="divide-y divide-(--border-soft)">
            {active.map((secret) => (
              <div key={secret.id} className="flex items-center gap-3 py-3 first:pt-0 last:pb-0">
                <div className="min-w-0 flex-1"><div className="truncate text-sm font-medium">{secret.name}</div><code className="text-xs text-(--color-text-subtle)">{secret.prefix}…</code></div>
                <div className="text-right text-[0.65rem] text-(--color-text-subtle)">{secret.last_used_at ? `Used ${new Date(secret.last_used_at).toLocaleDateString()}` : "Never used"}</div>
                {canRevoke && (
                  <Button size="sm" variant="ghost" disabled={revoke.isPending} onClick={() => revoke.mutate(secret.id)}>Revoke</Button>
                )}
              </div>
            ))}
          </div>
        )}
      </CardContent>
    </Card>
  )
}

function CreateTokenDialog({ userId, onClose }: { userId: string; onClose: () => void }) {
  const qc = useQueryClient()
  const [name, setName] = useState("EvoFlux connection")
  const [token, setToken] = useState<string | null>(null)
  const [copied, setCopied] = useState(false)
  const create = useMutation({
    mutationFn: () => api.createMemberSecret(userId, { name, scopes: CONNECTION_SECRET_SCOPES }),
    onSuccess: (result) => {
      setToken(result.token)
      void qc.invalidateQueries({ queryKey: MEMBER_QUERY_KEYS.secrets(userId) })
    },
  })
  return (
    <Dialog open title="Create connection token" onClose={onClose}>
      {token ? (
        <div className="space-y-3">
          <p className="text-sm text-(--color-text-muted)">Copy this token now. Conductor will not show it again.</p>
          <code className="block break-all rounded-lg border border-(--color-border) bg-(--bg-page) p-3 text-xs">{token}</code>
          <div className="flex justify-end gap-2">
            <Button variant="outline" onClick={() => void navigator.clipboard.writeText(token).then(() => setCopied(true))}>{copied ? <Check className="size-3.5" /> : <Copy className="size-3.5" />}{copied ? "Copied" : "Copy"}</Button>
            <Button onClick={onClose}>Done</Button>
          </div>
        </div>
      ) : (
        <div className="space-y-3">
          <div className="space-y-1.5"><Label>Token name</Label><Input value={name} onChange={(event) => setName(event.target.value)} /></div>
          <p className="text-xs text-(--color-text-subtle)">Includes resource sync, inventory sync, and privacy-safe usage reporting.</p>
          {create.error && <ErrorState message={create.error.message} />}
          <div className="flex justify-end gap-2"><Button variant="ghost" onClick={onClose}>Cancel</Button><Button variant="gradient" disabled={!name.trim() || create.isPending} onClick={() => create.mutate()}>Create token</Button></div>
        </div>
      )}
    </Dialog>
  )
}

function EditMemberDialog({ user, onClose, onSaved }: { user: User; onClose: () => void; onSaved: () => void }) {
  const actorId = useAuthStore((state) => state.user?.id)
  const [displayName, setDisplayName] = useState(user.display_name)
  const [role, setRole] = useState<PrimaryRole>(user.primary_role)
  const [subRoleIds, setSubRoleIds] = useState(user.sub_role_ids)
  const [tagIds, setTagIds] = useState(user.tag_ids)
  const tags = useQuery({ queryKey: ["tags"], queryFn: () => api.tags() })
  const subRoles = useQuery({ queryKey: ["sub-roles"], queryFn: () => api.subRoles() })
  const tagOptions = useMemo(() => (tags.data ?? []).map((item) => ({ value: item.id, label: item.name })), [tags.data])
  const subRoleOptions = useMemo(() => (subRoles.data ?? []).map((item) => ({ value: item.id, label: item.name })), [subRoles.data])
  const save = useMutation({
    mutationFn: () => api.updateMember(user.id, { display_name: displayName, primary_role: role, sub_role_ids: subRoleIds, tag_ids: tagIds }),
    onSuccess: onSaved,
  })
  return (
    <Dialog open title={`Edit ${user.email}`} onClose={onClose}>
      <div className="space-y-3">
        <div className="space-y-1.5"><Label>Display name</Label><Input value={displayName} onChange={(event) => setDisplayName(event.target.value)} /></div>
        <div className="space-y-1.5"><Label>Primary role</Label><Select value={role} disabled={actorId === user.id} onValueChange={(value) => setRole(value as PrimaryRole)} options={[...PRIMARY_ROLE_OPTIONS]} /></div>
        <div className="space-y-1.5"><Label>Sub-roles</Label><MultiSelect options={subRoleOptions} value={subRoleIds} onChange={setSubRoleIds} placeholder="Select sub-roles…" /></div>
        <div className="space-y-1.5"><Label>Tags</Label><MultiSelect options={tagOptions} value={tagIds} onChange={setTagIds} placeholder="Select tags…" /></div>
        {save.error && <ErrorState message={save.error.message} />}
        <div className="flex justify-end gap-2"><Button variant="ghost" onClick={onClose}>Cancel</Button><Button variant="gradient" disabled={!displayName.trim() || save.isPending} onClick={() => save.mutate()}>Save changes</Button></div>
      </div>
    </Dialog>
  )
}
