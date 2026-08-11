import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"
import { useNavigate, useParams } from "@tanstack/react-router"
import {
  Archive,
  ArrowLeft,
  Braces,
  CalendarClock,
  ExternalLink,
  LockKeyhole,
  MessageSquareText,
  Pencil,
  ShieldCheck,
  Star,
  Users,
} from "lucide-react"
import { useEffect, useMemo, useState } from "react"

import {
  api,
  type ManagedResource,
  type ResourceAccessPolicy,
} from "@/shared/api/client"
import {
  RESOURCE_KIND_LABEL,
  RESOURCE_QUERY_KEY,
  RESOURCE_STATUS,
} from "@/shared/constants/resource"
import { PageFrame } from "@/shared/components/page-frame"
import { useAuthStore } from "@/shared/stores/auth"
import { Badge } from "@/shared/ui/badge"
import { Button } from "@/shared/ui/button"
import { ConfirmDialog } from "@/shared/ui/dialog"
import { EmptyState, ErrorState } from "@/shared/ui/empty-state"
import { Input } from "@/shared/ui/input"
import { Label } from "@/shared/ui/label"
import { MultiSelect } from "@/shared/ui/multi-select"
import { Select } from "@/shared/ui/select"
import { SkeletonRows } from "@/shared/ui/skeleton"
import { Textarea } from "@/shared/ui/textarea"

export type ResourceGovernanceView = "overview" | "access" | "feedback"

export function ResourceGovernancePage({
  view,
}: {
  view: ResourceGovernanceView
}) {
  const { kind, resourceId } = useParams({ strict: false }) as {
    kind: string
    resourceId: string
  }
  const navigate = useNavigate()
  const actor = useAuthStore((state) => state.user)
  const resources = useQuery({
    queryKey: [RESOURCE_QUERY_KEY],
    queryFn: () => api.resources(),
  })
  const resource = resources.data?.find(
    (item) => item.id === resourceId && item.kind === kind,
  )
  const canManage = Boolean(
    resource &&
      actor &&
      (actor.primary_role === "admin" ||
        (actor.primary_role === "contribute" &&
          resource.owner_user_id === actor.id)),
  )

  const catalogPath = resourceCatalogPath(resource?.kind ?? kind)

  if (resources.isLoading) {
    return (
      <PageFrame title="Resource governance" subtitle="Loading the governed resource…">
        <SkeletonRows rows={7} />
      </PageFrame>
    )
  }

  if (resources.error || !resource) {
    return (
      <PageFrame
        title="Resource unavailable"
        subtitle="The resource may have been removed, or its access policy no longer includes you."
        action={
          <Button variant="outline" onClick={() => void navigate({ to: catalogPath })}>
            <ArrowLeft className="size-3.5" />
            Back to catalog
          </Button>
        }
      >
        <ErrorState
          message={
            resources.error instanceof Error
              ? resources.error.message
              : "This resource is not available to your account."
          }
        />
      </PageFrame>
    )
  }

  if (view === "access" && !canManage) {
    return (
      <PageFrame
        title={resource.name}
        subtitle={`${RESOURCE_KIND_LABEL[resource.kind]} · ${resource.slug}`}
        action={
          <Button
            variant="outline"
            onClick={() =>
              void navigate({
                to: "/app/resources/$kind/$resourceId",
                params: { kind: resource.kind, resourceId: resource.id },
              })
            }
          >
            <ArrowLeft className="size-3.5" />
            Resource overview
          </Button>
        }
      >
        <ErrorState message="Only the resource owner or a project admin can manage this access policy." />
      </PageFrame>
    )
  }

  return (
    <PageFrame
      title={resource.name}
      subtitle={`${RESOURCE_KIND_LABEL[resource.kind]} · ${resource.slug} · v${resource.version}`}
      className="max-w-7xl"
      action={
        <>
          <Button variant="outline" onClick={() => void navigate({ to: catalogPath })}>
            <ArrowLeft className="size-3.5" />
            Catalog
          </Button>
          {canManage && (
            <Button
              variant="gradient"
              onClick={() =>
                void navigate({
                  to: "/app/resources/$kind/$resourceId/edit",
                  params: { kind: resource.kind, resourceId: resource.id },
                })
              }
            >
              <ExternalLink className="size-3.5" />
              Open editor
            </Button>
          )}
        </>
      }
    >
      <ResourceGovernanceNav
        resource={resource}
        active={view}
        canManage={canManage}
      />

      {view === "overview" ? (
        <ResourceOverview resource={resource} canManage={canManage} />
      ) : view === "access" ? (
        <ResourceAccess resource={resource} />
      ) : (
        <ResourceFeedback resource={resource} canManage={canManage} />
      )}
    </PageFrame>
  )
}

function ResourceGovernanceNav({
  resource,
  active,
  canManage,
}: {
  resource: ManagedResource
  active: ResourceGovernanceView
  canManage: boolean
}) {
  const navigate = useNavigate()
  const tabs: Array<{
    value: ResourceGovernanceView
    label: string
    icon: typeof Braces
    managerOnly?: boolean
  }> = [
    { value: "overview", label: "Overview", icon: Braces },
    { value: "access", label: "Access", icon: ShieldCheck, managerOnly: true },
    { value: "feedback", label: "Feedback", icon: MessageSquareText },
  ]

  function open(next: ResourceGovernanceView) {
    const params = { kind: resource.kind, resourceId: resource.id }
    if (next === "overview") {
      void navigate({ to: "/app/resources/$kind/$resourceId", params })
      return
    }
    if (next === "access") {
      void navigate({ to: "/app/resources/$kind/$resourceId/access", params })
      return
    }
    void navigate({ to: "/app/resources/$kind/$resourceId/feedback", params })
  }

  return (
    <nav
      aria-label="Resource governance"
      className="mb-6 flex gap-1 overflow-x-auto border-b border-(--border-soft)"
    >
      {tabs
        .filter((tab) => !tab.managerOnly || canManage)
        .map((tab) => {
          const Icon = tab.icon
          return (
            <button
              key={tab.value}
              type="button"
              aria-current={active === tab.value ? "page" : undefined}
              onClick={() => open(tab.value)}
              className={`inline-flex shrink-0 items-center gap-1.5 border-b-2 px-3 py-2.5 text-sm font-medium transition-colors ${
                active === tab.value
                  ? "border-(--color-accent) text-(--color-text)"
                  : "border-transparent text-(--color-text-muted) hover:text-(--color-text)"
              }`}
            >
              <Icon className="size-3.5" />
              {tab.label}
            </button>
          )
        })}
    </nav>
  )
}

function ResourceOverview({
  resource,
  canManage,
}: {
  resource: ManagedResource
  canManage: boolean
}) {
  const queryClient = useQueryClient()
  const [name, setName] = useState(resource.name)
  const [description, setDescription] = useState(resource.description ?? "")
  const [visibility, setVisibility] = useState(resource.visibility)
  const [showArchive, setShowArchive] = useState(false)
  const [message, setMessage] = useState<string | null>(null)

  useEffect(() => {
    setName(resource.name)
    setDescription(resource.description ?? "")
    setVisibility(resource.visibility)
  }, [resource])

  const update = useMutation({
    mutationFn: () =>
      api.updateResource(resource.id, {
        name: name.trim(),
        description: description.trim(),
        visibility,
      }),
    onSuccess: (next) => {
      replaceCachedResource(queryClient, next)
      setMessage("Resource metadata saved.")
    },
  })

  const archive = useMutation({
    mutationFn: () => api.archiveResource(resource.id),
    onSuccess: (next) => {
      replaceCachedResource(queryClient, next)
      setShowArchive(false)
      setMessage("Resource archived. EvoFlux clients will remove it on their next sync.")
    },
  })

  const actionError = update.error ?? archive.error

  return (
    <div className="space-y-6">
      {(actionError || message) && (
        actionError ? (
          <ErrorState
            message={
              actionError instanceof Error ? actionError.message : "Resource update failed"
            }
          />
        ) : (
          <p
            role="status"
            className="rounded-lg border border-(--color-success)/30 bg-(--color-success)/8 px-3 py-2 text-sm text-(--color-success)"
          >
            {message}
          </p>
        )
      )}

      <section className="grid gap-3 sm:grid-cols-2 xl:grid-cols-5" aria-label="Resource summary">
        <SummaryItem label="Lifecycle" value={resource.status}>
          <ResourceStatusBadge status={resource.status} />
        </SummaryItem>
        <SummaryItem label="Current version" value={`v${resource.version}`} />
        <SummaryItem label="Release channel" value={resource.release_channel ?? "Not released"} />
        <SummaryItem label="Visibility" value={resource.visibility}>
          <span className="inline-flex items-center gap-1.5 capitalize">
            {resource.visibility === "private" ? (
              <LockKeyhole className="size-3.5" />
            ) : (
              <Users className="size-3.5" />
            )}
            {resource.visibility}
          </span>
        </SummaryItem>
        <SummaryItem label="Updated" value={formatDate(resource.updated_at)} />
      </section>

      <div className="grid gap-5 xl:grid-cols-[minmax(0,1fr)_22rem]">
        <section className="rounded-xl border border-(--border-card) bg-(--bg-card) p-5">
          <div className="mb-5 flex items-start gap-3">
            <span className="grid size-9 shrink-0 place-items-center rounded-lg bg-(--bg-key) text-(--color-text-muted)">
              <Pencil className="size-4" />
            </span>
            <div>
              <h2 className="text-sm font-semibold">Catalog metadata</h2>
              <p className="mt-0.5 text-xs text-(--color-text-muted)">
                The identity members see before installing or using this resource.
              </p>
            </div>
          </div>

          {canManage ? (
            <div className="grid gap-4 sm:grid-cols-2">
              <Field label="Name" htmlFor="resource-name">
                <Input
                  id="resource-name"
                  value={name}
                  maxLength={120}
                  onChange={(event) => {
                    setName(event.target.value)
                    setMessage(null)
                  }}
                />
              </Field>
              <Field label="Visibility" htmlFor="resource-visibility">
                <Select
                  id="resource-visibility"
                  value={visibility}
                  onValueChange={(next) => {
                    setVisibility(next)
                    setMessage(null)
                  }}
                  options={[
                    { value: "shared", label: "Shared" },
                    { value: "private", label: "Private" },
                  ]}
                />
              </Field>
              <div className="sm:col-span-2">
                <Field
                  label="Description"
                  htmlFor="resource-description"
                  hint={`${description.length}/1000 characters`}
                >
                  <Textarea
                    id="resource-description"
                    value={description}
                    maxLength={1000}
                    onChange={(event) => {
                      setDescription(event.target.value)
                      setMessage(null)
                    }}
                    placeholder="Explain when members should use this resource and what outcome it provides."
                    className="min-h-28"
                  />
                </Field>
              </div>
              <div className="flex justify-end sm:col-span-2">
                <Button
                  onClick={() => update.mutate()}
                  disabled={update.isPending || !name.trim()}
                >
                  {update.isPending ? "Saving…" : "Save metadata"}
                </Button>
              </div>
            </div>
          ) : (
            <p className="text-sm leading-6 text-(--color-text-muted)">
              {resource.description || "No description has been provided for this resource."}
            </p>
          )}
        </section>

        <aside className="space-y-4">
          <section className="rounded-xl border border-(--border-card) bg-(--bg-card) p-4">
            <h2 className="text-sm font-semibold">Release record</h2>
            <dl className="mt-3 space-y-3 text-xs">
              <Definition label="Highest version" value={resource.highest_version ? `v${resource.highest_version}` : "None"} />
              <Definition label="Published" value={resource.published_at ? formatDate(resource.published_at) : "Not published"} />
              <Definition label="Created" value={formatDate(resource.created_at)} />
              <Definition label="Draft revision" value={String(resource.draft_revision)} />
            </dl>
          </section>

          {canManage && (
            <section className="rounded-xl border border-(--color-error)/25 bg-(--color-error-subtle)/35 p-4">
              <div className="flex items-start gap-2.5">
                <Archive className="mt-0.5 size-4 shrink-0 text-(--color-error)" />
                <div>
                  <h2 className="text-sm font-semibold">Archive resource</h2>
                  <p className="mt-1 text-xs leading-relaxed text-(--color-text-muted)">
                    Removes the resource from EvoFlux clients while preserving versions,
                    feedback, and usage history for audit.
                  </p>
                </div>
              </div>
              <Button
                variant="destructive"
                size="sm"
                className="mt-4 w-full"
                disabled={resource.status === RESOURCE_STATUS.ARCHIVED}
                onClick={() => setShowArchive(true)}
              >
                <Archive className="size-3.5" />
                {resource.status === RESOURCE_STATUS.ARCHIVED ? "Already archived" : "Archive resource"}
              </Button>
            </section>
          )}
        </aside>
      </div>

      <section className="rounded-xl border border-(--border-card) bg-(--bg-card) p-5">
        <div className="mb-3 flex items-start gap-3">
          <span className="grid size-9 shrink-0 place-items-center rounded-lg bg-(--bg-key) text-(--color-text-muted)">
            <Braces className="size-4" />
          </span>
          <div>
            <h2 className="text-sm font-semibold">Published payload</h2>
            <p className="mt-0.5 text-xs text-(--color-text-muted)">
              Read-only catalog payload for the active resource record.
            </p>
          </div>
        </div>
        <pre className="max-h-96 overflow-auto rounded-lg border border-(--border-soft) bg-(--bg-page) p-4 font-mono text-xs leading-relaxed">
          {JSON.stringify(resource.payload, null, 2)}
        </pre>
      </section>

      <ConfirmDialog
        open={showArchive}
        title={`Archive ${resource.name}?`}
        description="EvoFlux clients will remove it immediately. Published versions, feedback, and usage history remain available for audit."
        confirmLabel="Archive resource"
        busy={archive.isPending}
        onClose={() => setShowArchive(false)}
        onConfirm={() => archive.mutate()}
      />
    </div>
  )
}

function ResourceAccess({ resource }: { resource: ManagedResource }) {
  const queryClient = useQueryClient()
  const access = useQuery({
    queryKey: [RESOURCE_QUERY_KEY, resource.id, "access"],
    queryFn: () => api.resourceAccess(resource.id),
  })
  const roles = useQuery({ queryKey: ["sub-roles"], queryFn: api.subRoles })
  const tags = useQuery({ queryKey: ["tags"], queryFn: api.tags })
  const members = useQuery({
    queryKey: ["members", "access-options"],
    queryFn: () => api.members({ status: "active", limit: 100 }),
  })
  const [policy, setPolicy] = useState<ResourceAccessPolicy | null>(null)
  const [saved, setSaved] = useState(false)

  useEffect(() => {
    if (access.data) setPolicy(access.data)
  }, [access.data])

  const save = useMutation({
    mutationFn: (next: ResourceAccessPolicy) => api.setResourceAccess(resource.id, next),
    onSuccess: (next) => {
      setPolicy(next)
      setSaved(true)
      queryClient.setQueryData([RESOURCE_QUERY_KEY, resource.id, "access"], next)
    },
  })

  const loading = access.isLoading || roles.isLoading || tags.isLoading || members.isLoading
  const loadError = access.error ?? roles.error ?? tags.error ?? members.error

  if (loadError) {
    return <ErrorState message={loadError instanceof Error ? loadError.message : "Access policy unavailable"} />
  }
  if (loading || !policy) return <SkeletonRows rows={6} />

  const noExplicitRules =
    !policy.all_members &&
    policy.primary_roles.length === 0 &&
    policy.sub_role_ids.length === 0 &&
    policy.tag_ids.length === 0 &&
    policy.member_ids.length === 0
  const sharedDefault = noExplicitRules && resource.visibility === "shared"
  const ownerOnly = noExplicitRules && resource.visibility === "private"

  function change(next: ResourceAccessPolicy) {
    setPolicy(next)
    setSaved(false)
  }

  return (
    <div className="grid gap-5 xl:grid-cols-[minmax(0,1fr)_20rem]">
      <section className="rounded-xl border border-(--border-card) bg-(--bg-card) p-5">
        <div className="mb-5 flex items-start gap-3">
          <span className="grid size-9 shrink-0 place-items-center rounded-lg bg-(--color-accent-soft) text-(--color-accent)">
            <ShieldCheck className="size-4" />
          </span>
          <div>
            <h2 className="text-sm font-semibold">Audience policy</h2>
            <p className="mt-0.5 text-xs text-(--color-text-muted)">
              Subjects are additive. A member receives the published resource when any rule matches.
            </p>
          </div>
        </div>

        {(save.error || saved) && (
          save.error ? (
            <ErrorState
              className="mb-4"
              message={save.error instanceof Error ? save.error.message : "Access update failed"}
            />
          ) : (
            <p role="status" className="mb-4 rounded-lg border border-(--color-success)/30 bg-(--color-success)/8 px-3 py-2 text-sm text-(--color-success)">
              Access policy saved and queued for connected EvoFlux clients.
            </p>
          )
        )}

        <label className="flex cursor-pointer items-start gap-3 rounded-xl border border-(--border-soft) bg-(--bg-page) p-4">
          <input
            type="checkbox"
            checked={policy.all_members}
            onChange={(event) => change({ ...policy, all_members: event.target.checked })}
            className="mt-0.5 size-4 accent-(--color-accent)"
          />
          <span>
            <span className="block text-sm font-medium">All active members</span>
            <span className="mt-0.5 block text-xs leading-relaxed text-(--color-text-muted)">
              Explicitly grant access to every active member, independent of role, tag, or team.
            </span>
          </span>
        </label>

        <div className="mt-5 grid gap-4 sm:grid-cols-2">
          <Field label="Primary roles" htmlFor="access-primary-roles">
            <MultiSelect
              id="access-primary-roles"
              disabled={policy.all_members}
              options={[
                { value: "admin", label: "Admin" },
                { value: "contribute", label: "Contribute" },
                { value: "user", label: "User" },
              ]}
              value={policy.primary_roles}
              onChange={(primary_roles) => change({ ...policy, primary_roles })}
            />
          </Field>
          <Field label="Sub-roles" htmlFor="access-subroles">
            <MultiSelect
              id="access-subroles"
              disabled={policy.all_members}
              options={(roles.data ?? []).map((role) => ({ value: role.id, label: role.name }))}
              value={policy.sub_role_ids}
              onChange={(sub_role_ids) => change({ ...policy, sub_role_ids })}
            />
          </Field>
          <Field label="Member tags" htmlFor="access-tags">
            <MultiSelect
              id="access-tags"
              disabled={policy.all_members}
              options={(tags.data ?? []).map((tag) => ({ value: tag.id, label: tag.name }))}
              value={policy.tag_ids}
              onChange={(tag_ids) => change({ ...policy, tag_ids })}
            />
          </Field>
          <Field label="Specific members" htmlFor="access-members">
            <MultiSelect
              id="access-members"
              disabled={policy.all_members}
              options={(members.data?.items ?? []).map((member) => ({
                value: member.id,
                label: `${member.display_name} · ${member.email}`,
              }))}
              value={policy.member_ids}
              onChange={(member_ids) => change({ ...policy, member_ids })}
            />
          </Field>
        </div>

        <div className="mt-5 flex justify-end">
          <Button onClick={() => save.mutate(policy)} disabled={save.isPending}>
            <ShieldCheck className="size-3.5" />
            {save.isPending ? "Saving…" : "Save access policy"}
          </Button>
        </div>
      </section>

      <aside className="space-y-4">
        <section className="rounded-xl border border-(--border-card) bg-(--bg-card) p-4">
          <h2 className="text-sm font-semibold">Effective audience</h2>
          <p className="mt-2 text-xs leading-relaxed text-(--color-text-muted)">
            {policy.all_members
              ? "Every active project member is included explicitly."
              : sharedDefault
                ? "Every active member is included by the shared-resource default because no explicit rules exist."
                : ownerOnly
                  ? "Only the owner can receive this private resource because no explicit rules exist."
                  : "Members matching at least one selected subject can receive the published resource."}
          </p>
        </section>
        <section className="rounded-xl border border-(--border-card) bg-(--bg-card) p-4">
          <h2 className="text-sm font-semibold">Policy behavior</h2>
          <ul className="mt-2 space-y-2 text-xs leading-relaxed text-(--color-text-muted)">
            <li>• Draft source remains owner/admin only.</li>
            <li>• Access revocation is pushed to connected clients in real time.</li>
            <li>• Published version history and telemetry remain auditable.</li>
          </ul>
        </section>
      </aside>
    </div>
  )
}

function ResourceFeedback({
  resource,
  canManage,
}: {
  resource: ManagedResource
  canManage: boolean
}) {
  const queryClient = useQueryClient()
  const [rating, setRating] = useState<"1" | "2" | "3" | "4" | "5">("5")
  const [comment, setComment] = useState("")
  const [submitted, setSubmitted] = useState(false)
  const feedback = useQuery({
    queryKey: [RESOURCE_QUERY_KEY, resource.id, "feedback"],
    queryFn: () => api.resourceFeedback(resource.id),
    enabled: canManage,
  })
  const submit = useMutation({
    mutationFn: () => api.submitResourceFeedback(resource.id, Number(rating), comment.trim()),
    onSuccess: () => {
      setSubmitted(true)
      setComment("")
      void queryClient.invalidateQueries({
        queryKey: [RESOURCE_QUERY_KEY, resource.id, "feedback"],
      })
      void queryClient.invalidateQueries({
        queryKey: [RESOURCE_QUERY_KEY, resource.id, "monitoring"],
      })
    },
  })
  const feedbackOpen =
    resource.status === RESOURCE_STATUS.PUBLISHED ||
    resource.status === RESOURCE_STATUS.BETA
  const summary = useMemo(() => {
    const items = feedback.data ?? []
    if (items.length === 0) return { count: 0, average: null as number | null }
    return {
      count: items.length,
      average: items.reduce((total, item) => total + item.rating, 0) / items.length,
    }
  }, [feedback.data])

  return (
    <div className="grid gap-5 xl:grid-cols-[minmax(0,0.9fr)_minmax(24rem,1.1fr)]">
      <section className="rounded-xl border border-(--border-card) bg-(--bg-card) p-5">
        <div className="mb-5 flex items-start gap-3">
          <span className="grid size-9 shrink-0 place-items-center rounded-lg bg-(--color-warning)/10 text-(--color-warning)">
            <Star className="size-4" />
          </span>
          <div>
            <h2 className="text-sm font-semibold">Rate this version</h2>
            <p className="mt-0.5 text-xs text-(--color-text-muted)">
              One current response per member and version. Submitting again updates your response.
            </p>
          </div>
        </div>

        {(submit.error || submitted) && (
          submit.error ? (
            <ErrorState
              className="mb-4"
              message={submit.error instanceof Error ? submit.error.message : "Feedback submission failed"}
            />
          ) : (
            <p role="status" className="mb-4 rounded-lg border border-(--color-success)/30 bg-(--color-success)/8 px-3 py-2 text-sm text-(--color-success)">
              Your feedback for v{resource.version} was saved.
            </p>
          )
        )}

        {feedbackOpen ? (
          <div className="space-y-4">
            <Field label="Rating" htmlFor="feedback-rating">
              <Select
                id="feedback-rating"
                value={rating}
                onValueChange={(next) => {
                  setRating(next)
                  setSubmitted(false)
                }}
                options={[
                  { value: "5", label: "5 — Excellent" },
                  { value: "4", label: "4 — Good" },
                  { value: "3", label: "3 — Okay" },
                  { value: "2", label: "2 — Poor" },
                  { value: "1", label: "1 — Blocked" },
                ]}
              />
            </Field>
            <Field
              label="Comment"
              htmlFor="feedback-comment"
              hint={`${comment.length}/1000 characters`}
            >
              <Textarea
                id="feedback-comment"
                value={comment}
                maxLength={1000}
                onChange={(event) => {
                  setComment(event.target.value)
                  setSubmitted(false)
                }}
                placeholder="What worked, what failed, and what should change?"
                className="min-h-32"
              />
            </Field>
            <div className="flex justify-end">
              <Button onClick={() => submit.mutate()} disabled={submit.isPending}>
                <Star className="size-3.5" />
                {submit.isPending ? "Submitting…" : "Submit feedback"}
              </Button>
            </div>
          </div>
        ) : (
          <EmptyState
            icon={CalendarClock}
            title="Feedback is not open"
            description="Feedback becomes available when a beta or published version is released."
            className="py-8"
          />
        )}
      </section>

      {canManage ? (
        <section className="rounded-xl border border-(--border-card) bg-(--bg-card) p-5">
          <div className="mb-4 flex flex-wrap items-start justify-between gap-3">
            <div>
              <h2 className="text-sm font-semibold">Member feedback</h2>
              <p className="mt-0.5 text-xs text-(--color-text-muted)">
                Qualitative evidence attached to immutable resource versions.
              </p>
            </div>
            <div className="flex gap-2">
              <Badge>{summary.count} responses</Badge>
              <Badge tone={summary.average && summary.average >= 4 ? "success" : "warning"}>
                {summary.average ? `${summary.average.toFixed(1)} avg` : "No rating"}
              </Badge>
            </div>
          </div>
          {feedback.error ? (
            <ErrorState message={feedback.error instanceof Error ? feedback.error.message : "Feedback unavailable"} />
          ) : feedback.isLoading ? (
            <SkeletonRows rows={4} />
          ) : (feedback.data?.length ?? 0) === 0 ? (
            <EmptyState
              icon={MessageSquareText}
              title="No feedback yet"
              description="Invite early adopters to rate the released version after real work."
              className="py-8"
            />
          ) : (
            <div className="space-y-2">
              {feedback.data?.map((item) => (
                <article key={item.id} className="rounded-lg border border-(--border-soft) bg-(--bg-page) p-3.5">
                  <div className="flex flex-wrap items-center justify-between gap-2">
                    <span className="text-sm font-medium">{item.member_name}</span>
                    <span className="inline-flex items-center gap-1 text-xs font-medium text-(--color-warning)">
                      <Star className="size-3 fill-current" />
                      {item.rating}/5 · v{item.resource_version}
                    </span>
                  </div>
                  {item.comment && (
                    <p className="mt-2 text-sm leading-relaxed text-(--color-text-muted)">
                      {item.comment}
                    </p>
                  )}
                  <p className="mt-2 text-[0.7rem] text-(--color-text-subtle)">
                    Updated {formatDate(item.updated_at)}
                  </p>
                </article>
              ))}
            </div>
          )}
        </section>
      ) : (
        <aside className="rounded-xl border border-(--border-card) bg-(--bg-card) p-5">
          <h2 className="text-sm font-semibold">How feedback is used</h2>
          <p className="mt-2 text-sm leading-relaxed text-(--color-text-muted)">
            Resource owners use member feedback alongside usage and reliability data to
            decide what to improve in the next release. Other members’ responses remain
            visible only to the resource owner and project admins.
          </p>
        </aside>
      )}
    </div>
  )
}

function SummaryItem({
  label,
  value,
  children,
}: {
  label: string
  value: string
  children?: React.ReactNode
}) {
  return (
    <div className="rounded-xl border border-(--border-card) bg-(--bg-card) px-4 py-3">
      <p className="text-[0.7rem] font-medium tracking-wide text-(--color-text-subtle) uppercase">
        {label}
      </p>
      <div className="mt-1.5 min-w-0 truncate text-sm font-semibold capitalize" title={value}>
        {children ?? value}
      </div>
    </div>
  )
}

function Definition({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-start justify-between gap-4 border-b border-(--border-soft) pb-2 last:border-0 last:pb-0">
      <dt className="text-(--color-text-muted)">{label}</dt>
      <dd className="text-right font-medium text-(--color-text)">{value}</dd>
    </div>
  )
}

function Field({
  label,
  htmlFor,
  hint,
  children,
}: {
  label: string
  htmlFor: string
  hint?: string
  children: React.ReactNode
}) {
  return (
    <div className="space-y-1.5">
      <Label htmlFor={htmlFor}>{label}</Label>
      {children}
      {hint && <p className="text-[0.7rem] text-(--color-text-subtle)">{hint}</p>}
    </div>
  )
}

function ResourceStatusBadge({ status }: { status: ManagedResource["status"] }) {
  const tone =
    status === RESOURCE_STATUS.PUBLISHED
      ? "success"
      : status === RESOURCE_STATUS.DRAFT || status === RESOURCE_STATUS.BETA
        ? "warning"
        : "neutral"
  return (
    <Badge tone={tone} className="capitalize">
      {status}
    </Badge>
  )
}

function replaceCachedResource(
  queryClient: ReturnType<typeof useQueryClient>,
  resource: ManagedResource,
) {
  queryClient.setQueryData<ManagedResource[]>([RESOURCE_QUERY_KEY], (current) =>
    current?.map((item) => (item.id === resource.id ? resource : item)),
  )
}

function resourceCatalogPath(kind: string) {
  if (kind === "agent") return "/app/resources/agents" as const
  if (kind === "skill") return "/app/resources/skills" as const
  if (kind === "plugin") return "/app/resources/plugins" as const
  return "/app/resources" as const
}

function formatDate(value: string) {
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(new Date(value))
}
