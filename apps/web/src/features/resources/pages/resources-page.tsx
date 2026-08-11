import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"
import { useNavigate } from "@tanstack/react-router"
import {
  Activity,
  Archive,
  Boxes,
  CheckCircle2,
  GitBranch,
  LockKeyhole,
  MessageSquareText,
  Pencil,
  Plus,
  Search,
  ShieldCheck,
  Star,
  Users,
} from "lucide-react"
import { useEffect, useMemo, useRef, useState } from "react"

import {
  api,
  type ManagedResource,
  type ResourceAccessPolicy,
  type ResourceMonitoring,
  type ResourceVersion,
} from "@/shared/api/client"
import { PageFrame } from "@/shared/components/page-frame"
import { useAuthStore } from "@/shared/stores/auth"
import { Badge } from "@/shared/ui/badge"
import { Button } from "@/shared/ui/button"
import { Card, CardContent } from "@/shared/ui/card"
import { ConfirmDialog, Dialog } from "@/shared/ui/dialog"
import { EmptyState, ErrorState } from "@/shared/ui/empty-state"
import { Input } from "@/shared/ui/input"
import { Label } from "@/shared/ui/label"
import { MultiSelect } from "@/shared/ui/multi-select"
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
import { Textarea } from "@/shared/ui/textarea"
import {
  RESOURCE_KIND,
  RESOURCE_KIND_LABEL,
  RESOURCE_KIND_OPTIONS,
  RESOURCE_QUERY_KEY,
} from "@/shared/constants/resource"

type ResourceKind = ManagedResource["kind"]
type ResourceStatus = ManagedResource["status"]
type DetailTab = "overview" | "versions" | "access" | "monitoring" | "feedback"

const kindOptions = [
  { value: "all", label: "All types" },
  ...RESOURCE_KIND_OPTIONS,
] as const

const kindLabels = RESOURCE_KIND_LABEL

const kindPageMeta: Partial<Record<ResourceKind, { title: string; subtitle: string }>> = {
  [RESOURCE_KIND.PLUGIN]: {
    title: "Plugins",
    subtitle: "Portable Plugins published to EvoFlux clients, from Draft to measurable outcomes.",
  },
  skill: {
    title: "Skills",
    subtitle: "Govern reusable skills from draft to measurable outcomes.",
  },
  agent: {
    title: "Agents",
    subtitle: "Govern agent definitions from draft to measurable outcomes.",
  },
}

const statusOptions = [
  { value: "all", label: "All status" },
  { value: "published", label: "Published" },
  { value: "draft", label: "Draft" },
  { value: "archived", label: "Archived" },
] as const

const resourceKindOptions = kindOptions.slice(1) as ReadonlyArray<{
  value: ResourceKind
  label: string
}>

export function ResourcesPage({ fixedKind }: { fixedKind?: ResourceKind }) {
  const qc = useQueryClient()
  const navigate = useNavigate()
  const user = useAuthStore((state) => state.user)
  const canCreate = user?.primary_role === "admin" || user?.primary_role === "contribute"
  const { data = [], isLoading, error } = useQuery({
    queryKey: [RESOURCE_QUERY_KEY],
    queryFn: () => api.resources(),
  })
  const [query, setQuery] = useState("")
  const [kind, setKind] = useState<(typeof kindOptions)[number]["value"]>(fixedKind ?? "all")
  const [status, setStatus] = useState<(typeof statusOptions)[number]["value"]>("all")
  const [showCreate, setShowCreate] = useState(false)
  const [selected, setSelected] = useState<ManagedResource | null>(null)
  const [pendingArchive, setPendingArchive] = useState<ManagedResource | null>(null)
  const searchRef = useRef<HTMLInputElement | null>(null)

  const archive = useMutation({
    mutationFn: (id: string) => api.archiveResource(id),
    onSuccess: () => {
      setPendingArchive(null)
      setSelected(null)
      void qc.invalidateQueries({ queryKey: [RESOURCE_QUERY_KEY] })
      focusAfterDialogTransition(() => searchRef.current?.focus())
    },
  })

  const filtered = useMemo(() => {
    const term = query.trim().toLowerCase()
    return data.filter(
      (resource) =>
        (kind === "all" || resource.kind === kind) &&
        (status === "all" || resource.status === status) &&
        (!term ||
          resource.name.toLowerCase().includes(term) ||
          resource.slug.toLowerCase().includes(term)),
    )
  }, [data, kind, query, status])

  const published = data.filter((resource) => resource.status === "published").length
  const drafts = data.filter((resource) => resource.status === "draft").length
  const catalogKinds = new Set(data.map((resource) => resource.kind)).size

  function canManage(resource: ManagedResource) {
    return (
      user?.primary_role === "admin" ||
      (user?.primary_role === "contribute" && resource.owner_user_id === user.id)
    )
  }

  const meta = (fixedKind && kindPageMeta[fixedKind]) || {
    title: "Resource catalog",
    subtitle:
      "Govern agents, skills, plugins and reusable workflows from draft to measurable outcomes.",
  }

  return (
    <PageFrame
      title={meta.title}
      subtitle={meta.subtitle}
      action={
        canCreate ? (
          <Button variant="gradient" onClick={() => setShowCreate(true)}>
            <Plus className="size-3.5" />
            New resource
          </Button>
        ) : undefined
      }
    >
      <div className="mb-5 grid gap-3 sm:grid-cols-3">
        <CatalogMetric label="Published" value={published} icon={CheckCircle2} tone="success" />
        <CatalogMetric label="Drafts" value={drafts} icon={Pencil} tone="warning" />
        <CatalogMetric label="Resource types" value={catalogKinds} icon={Boxes} tone="accent" />
      </div>

      <div className="mb-4 flex flex-col gap-2 sm:flex-row">
        <div className="relative min-w-0 flex-1">
          <Search className="pointer-events-none absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2 text-(--color-text-subtle)" />
          <Input
            ref={searchRef}
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder="Search by name or slug…"
            aria-label="Search resources"
            className="pl-8"
          />
        </div>
        {fixedKind === undefined && (
          <Select value={kind} onValueChange={setKind} options={kindOptions} className="sm:w-40" />
        )}
        <Select
          value={status}
          onValueChange={setStatus}
          options={statusOptions}
          className="sm:w-40"
        />
      </div>

      {(error || archive.error) && (
        <ErrorState
          className="mb-4"
          message={
            error instanceof Error
              ? error.message
              : archive.error instanceof Error
                ? archive.error.message
                : "Catalog action failed"
          }
        />
      )}

      {isLoading ? (
        <TableWrap>
          <SkeletonRows rows={5} />
        </TableWrap>
      ) : filtered.length === 0 ? (
        <EmptyState
          icon={Boxes}
          title={data.length === 0 ? "Build your governed catalog" : "No matching resources"}
          description={
            data.length === 0
              ? "Create a draft, define access, publish a version, then monitor how members use it."
              : "Try another search, type, or status filter."
          }
          action={
            data.length === 0 && canCreate ? (
              <Button variant="outline" onClick={() => setShowCreate(true)}>
                Create first resource
              </Button>
            ) : undefined
          }
        />
      ) : (
        <TableWrap>
          <Table>
            <TableHead>
              <tr>
                <TableTh>Resource</TableTh>
                <TableTh>Type</TableTh>
                <TableTh>Status</TableTh>
                <TableTh>Version</TableTh>
                <TableTh>Access</TableTh>
                <TableTh>Updated</TableTh>
                <TableTh />
              </tr>
            </TableHead>
            <TableBody>
              {filtered.map((resource) => (
                <TableRow key={resource.id}>
                  <TableTd>
                    <div className="font-medium">{resource.name}</div>
                    <div className="max-w-xs truncate font-mono text-[0.7rem] text-(--color-text-subtle)">
                      {resource.slug}
                    </div>
                  </TableTd>
                  <TableTd>
                    <Badge>{kindLabels[resource.kind]}</Badge>
                  </TableTd>
                  <TableTd>
                    <StatusBadge status={resource.status} />
                  </TableTd>
                  <TableTd className="font-mono text-xs text-(--color-text-muted)">
                    {resource.version}
                  </TableTd>
                  <TableTd>
                    <span className="inline-flex items-center gap-1 text-xs capitalize text-(--color-text-muted)">
                      {resource.visibility === "private" ? (
                        <LockKeyhole className="size-3" />
                      ) : (
                        <Users className="size-3" />
                      )}
                      {resource.visibility}
                    </span>
                  </TableTd>
                  <TableTd className="text-xs text-(--color-text-muted)">
                    {formatDate(resource.updated_at)}
                  </TableTd>
                  <TableTd className="text-right">
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={() =>
                        void navigate({
                          to: "/app/resources/$kind/$resourceId/edit",
                          params: { kind: resource.kind, resourceId: resource.id },
                        })
                      }
                    >
                      {canManage(resource) ? "Manage" : "View"}
                    </Button>
                  </TableTd>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </TableWrap>
      )}

      <CreateResourceDialog
        open={showCreate}
        defaultKind={fixedKind}
        onClose={() => setShowCreate(false)}
        onCreated={(resource) => {
          setShowCreate(false)
          void qc.invalidateQueries({ queryKey: [RESOURCE_QUERY_KEY] })
          void navigate({
            to: "/app/resources/$kind/$resourceId/edit",
            params: { kind: resource.kind, resourceId: resource.id },
          })
        }}
      />
      {selected && (
        <ResourceWorkspace
          resource={selected}
          canManage={canManage(selected)}
          onClose={() => setSelected(null)}
          onChanged={(resource) => {
            setSelected(resource)
            void qc.invalidateQueries({ queryKey: [RESOURCE_QUERY_KEY] })
          }}
          onArchive={() => setPendingArchive(selected)}
        />
      )}
      <ConfirmDialog
        open={pendingArchive !== null}
        title={`Archive ${pendingArchive?.name ?? "resource"}?`}
        description="EvoFlux clients will remove it immediately. Usage history and versions remain available for audit."
        confirmLabel="Archive resource"
        busy={archive.isPending}
        onClose={() => setPendingArchive(null)}
        onConfirm={() => pendingArchive && archive.mutate(pendingArchive.id)}
      />
    </PageFrame>
  )
}

function CatalogMetric({
  label,
  value,
  icon: Icon,
  tone,
}: {
  label: string
  value: number
  icon: typeof Boxes
  tone: "success" | "warning" | "accent"
}) {
  const colors = {
    success: "text-(--color-success) bg-(--color-success)/10",
    warning: "text-(--color-warning) bg-(--color-warning)/10",
    accent: "text-(--color-accent) bg-(--color-accent-soft)",
  }
  return (
    <Card>
      <CardContent className="flex items-center gap-3">
        <span className={`grid size-9 place-items-center rounded-lg ${colors[tone]}`}>
          <Icon className="size-4" />
        </span>
        <div>
          <div className="text-xl font-semibold tabular-nums">{value}</div>
          <div className="text-xs text-(--color-text-muted)">{label}</div>
        </div>
      </CardContent>
    </Card>
  )
}

function CreateResourceDialog({
  open,
  defaultKind,
  onClose,
  onCreated,
}: {
  open: boolean
  defaultKind?: ResourceKind
  onClose: () => void
  onCreated: (resource: ManagedResource) => void
}) {
  const [kind, setKind] = useState<ResourceKind>(defaultKind ?? "agent")
  const [slug, setSlug] = useState("")
  const [name, setName] = useState("")
  const [description, setDescription] = useState("")
  const [version, setVersion] = useState("0.1.0")
  const [visibility, setVisibility] = useState<ManagedResource["visibility"]>("shared")
  const [payload, setPayload] = useState("{}")
  const [changelog, setChangelog] = useState("Initial version")
  const [localError, setLocalError] = useState<string | null>(null)

  const create = useMutation({
    mutationFn: () => {
      let parsed: unknown
      try {
        parsed = JSON.parse(payload)
      } catch {
        throw new Error("Payload must be valid JSON")
      }
      return api.createResource({
        kind,
        slug,
        name,
        description,
        version,
        visibility,
        payload: parsed,
        changelog,
      })
    },
    onSuccess: onCreated,
  })

  return (
    <Dialog
      open={open}
      title="Create resource draft"
      description="Start with a versioned draft. Nothing reaches EvoFlux until you publish it."
      onClose={onClose}
      className="sm:max-w-2xl"
      footer={
        <>
          <Button variant="ghost" onClick={onClose} disabled={create.isPending}>
            Cancel
          </Button>
          <Button
            variant="gradient"
            disabled={create.isPending || !slug.trim() || !name.trim()}
            onClick={() => {
              setLocalError(null)
              create.mutate()
            }}
          >
            {create.isPending ? "Creating…" : "Create draft"}
          </Button>
        </>
      }
    >
      {(localError || create.error) && (
        <ErrorState
          className="mb-4"
          message={localError ?? (create.error instanceof Error ? create.error.message : "Create failed")}
        />
      )}
      <div className="grid gap-4 sm:grid-cols-2">
        <Field label="Type" htmlFor="resource-kind">
          <Select
            id="resource-kind"
            value={kind}
            onValueChange={setKind}
            options={resourceKindOptions}
          />
        </Field>
        <Field label="Initial version" htmlFor="resource-version">
          <Input id="resource-version" value={version} onChange={(e) => setVersion(e.target.value)} />
        </Field>
        <Field label="Name" htmlFor="resource-name">
          <Input id="resource-name" value={name} onChange={(e) => setName(e.target.value)} autoFocus />
        </Field>
        <Field label="Slug" htmlFor="resource-slug" hint="Lowercase letters, numbers and hyphens">
          <Input
            id="resource-slug"
            value={slug}
            onChange={(e) => setSlug(e.target.value.toLowerCase().replace(/[^a-z0-9-]/g, "-"))}
          />
        </Field>
        <div className="sm:col-span-2">
          <Field label="Description" htmlFor="resource-description">
            <Textarea
              id="resource-description"
              value={description}
              onChange={(e) => setDescription(e.target.value)}
            />
          </Field>
        </div>
        <Field label="Default visibility" htmlFor="resource-visibility">
          <Select
            id="resource-visibility"
            value={visibility}
            onValueChange={setVisibility}
            options={[
              { value: "shared", label: "Shared — all members by default" },
              { value: "private", label: "Private — owner only by default" },
            ]}
          />
        </Field>
        <Field label="Changelog" htmlFor="resource-changelog">
          <Input id="resource-changelog" value={changelog} onChange={(e) => setChangelog(e.target.value)} />
        </Field>
        <div className="sm:col-span-2">
          <Field label="Version payload (JSON)" htmlFor="resource-payload">
            <Textarea
              id="resource-payload"
              value={payload}
              onChange={(e) => setPayload(e.target.value)}
              className="min-h-36 font-mono text-xs"
              spellCheck={false}
            />
          </Field>
        </div>
      </div>
    </Dialog>
  )
}

function ResourceWorkspace({
  resource,
  canManage,
  onClose,
  onChanged,
  onArchive,
}: {
  resource: ManagedResource
  canManage: boolean
  onClose: () => void
  onChanged: (resource: ManagedResource) => void
  onArchive: () => void
}) {
  const [tab, setTab] = useState<DetailTab>("overview")
  const tabs: Array<{ value: DetailTab; label: string; managerOnly?: boolean }> = [
    { value: "overview", label: "Overview" },
    { value: "versions", label: "Versions", managerOnly: true },
    { value: "access", label: "Access", managerOnly: true },
    { value: "monitoring", label: "Monitoring", managerOnly: true },
    { value: "feedback", label: "Feedback" },
  ]

  return (
    <Dialog
      open
      title={resource.name}
      description={`${kindLabels[resource.kind]} · ${resource.slug} · v${resource.version}`}
      onClose={onClose}
      className="sm:max-w-4xl"
    >
      <div className="mb-5 flex gap-1 overflow-x-auto border-b border-(--border-soft) pb-px">
        {tabs
          .filter((item) => !item.managerOnly || canManage)
          .map((item) => (
            <button
              key={item.value}
              type="button"
              onClick={() => setTab(item.value)}
              className={`shrink-0 border-b-2 px-3 py-2 text-xs font-medium transition-colors ${
                tab === item.value
                  ? "border-(--color-accent) text-(--color-text)"
                  : "border-transparent text-(--color-text-muted) hover:text-(--color-text)"
              }`}
            >
              {item.label}
            </button>
          ))}
      </div>

      {tab === "overview" && (
        <OverviewPanel
          resource={resource}
          canManage={canManage}
          onChanged={onChanged}
          onArchive={onArchive}
        />
      )}
      {tab === "versions" && canManage && (
        <VersionsPanel resource={resource} onChanged={onChanged} />
      )}
      {tab === "access" && canManage && <AccessPanel resource={resource} />}
      {tab === "monitoring" && canManage && <MonitoringPanel resource={resource} />}
      {tab === "feedback" && <FeedbackPanel resource={resource} canManage={canManage} />}
    </Dialog>
  )
}

function OverviewPanel({
  resource,
  canManage,
  onChanged,
  onArchive,
}: {
  resource: ManagedResource
  canManage: boolean
  onChanged: (resource: ManagedResource) => void
  onArchive: () => void
}) {
  const [name, setName] = useState(resource.name)
  const [description, setDescription] = useState(resource.description ?? "")
  const [visibility, setVisibility] = useState(resource.visibility)
  useEffect(() => {
    setName(resource.name)
    setDescription(resource.description ?? "")
    setVisibility(resource.visibility)
  }, [resource])

  const update = useMutation({
    mutationFn: () => api.updateResource(resource.id, { name, description, visibility }),
    onSuccess: onChanged,
  })

  return (
    <div className="space-y-5">
      <div className="grid gap-3 sm:grid-cols-3">
        <MiniMetric label="Lifecycle" value={resource.status} />
        <MiniMetric label="Published version" value={`v${resource.version}`} />
        <MiniMetric label="Last updated" value={formatDate(resource.updated_at)} />
      </div>

      {canManage ? (
        <div className="rounded-xl border border-(--border-card) p-4">
          <div className="mb-3 flex items-center gap-2">
            <Pencil className="size-4 text-(--color-text-subtle)" />
            <h3 className="text-sm font-medium">Catalog metadata</h3>
          </div>
          {update.error && (
            <ErrorState className="mb-3" message={update.error instanceof Error ? update.error.message : "Update failed"} />
          )}
          <div className="grid gap-4 sm:grid-cols-2">
            <Field label="Name" htmlFor="edit-resource-name">
              <Input id="edit-resource-name" value={name} onChange={(e) => setName(e.target.value)} />
            </Field>
            <Field label="Visibility" htmlFor="edit-resource-visibility">
              <Select
                id="edit-resource-visibility"
                value={visibility}
                onValueChange={setVisibility}
                options={[
                  { value: "shared", label: "Shared" },
                  { value: "private", label: "Private" },
                ]}
              />
            </Field>
            <div className="sm:col-span-2">
              <Field label="Description" htmlFor="edit-resource-description">
                <Textarea
                  id="edit-resource-description"
                  value={description}
                  onChange={(e) => setDescription(e.target.value)}
                />
              </Field>
            </div>
          </div>
          <div className="mt-4 flex flex-wrap justify-between gap-2">
            <Button
              variant="destructive"
              size="sm"
              onClick={onArchive}
              disabled={resource.status === "archived"}
            >
              <Archive className="size-3.5" />
              Archive
            </Button>
            <Button
              size="sm"
              onClick={() => update.mutate()}
              disabled={update.isPending || !name.trim()}
            >
              {update.isPending ? "Saving…" : "Save metadata"}
            </Button>
          </div>
        </div>
      ) : (
        <p className="text-sm leading-relaxed text-(--color-text-muted)">
          {resource.description || "No description provided."}
        </p>
      )}

      <div>
        <h3 className="mb-2 text-sm font-medium">Published payload</h3>
        <pre className="max-h-72 overflow-auto rounded-lg border border-(--border-soft) bg-(--bg-page) p-3 font-mono text-xs leading-relaxed">
          {JSON.stringify(resource.payload, null, 2)}
        </pre>
      </div>
    </div>
  )
}

function VersionsPanel({
  resource,
  onChanged,
}: {
  resource: ManagedResource
  onChanged: (resource: ManagedResource) => void
}) {
  const qc = useQueryClient()
  const [showNew, setShowNew] = useState(false)
  const [version, setVersion] = useState(nextPatch(resource.version))
  const [payload, setPayload] = useState(JSON.stringify(resource.payload, null, 2))
  const [changelog, setChangelog] = useState("")
  const [pendingPublish, setPendingPublish] = useState<ResourceVersion | null>(null)
  const newVersionButtonRef = useRef<HTMLButtonElement | null>(null)
  const { data = [], isLoading, error } = useQuery({
    queryKey: ["resources", resource.id, "versions"],
    queryFn: () => api.resourceVersions(resource.id),
  })
  const create = useMutation({
    mutationFn: () => {
      let parsed: unknown
      try {
        parsed = JSON.parse(payload)
      } catch {
        throw new Error("Payload must be valid JSON")
      }
      return api.createResourceVersion(resource.id, { version, payload: parsed, changelog })
    },
    onSuccess: () => {
      setShowNew(false)
      void qc.invalidateQueries({ queryKey: ["resources", resource.id, "versions"] })
    },
  })
  const publish = useMutation({
    mutationFn: (versionId: string) => api.publishResourceVersion(resource.id, versionId),
    onSuccess: (updated) => {
      setPendingPublish(null)
      onChanged(updated)
      void qc.invalidateQueries({ queryKey: ["resources", resource.id, "versions"] })
      focusAfterDialogTransition(() => newVersionButtonRef.current?.focus())
    },
  })

  return (
    <div className="space-y-4">
      <div className="flex items-start justify-between gap-3">
        <div>
          <h3 className="text-sm font-medium">Version history</h3>
          <p className="mt-0.5 text-xs text-(--color-text-muted)">
            Drafts are editable artifacts; publishing atomically updates connected EvoFlux clients.
          </p>
        </div>
        <Button
          ref={newVersionButtonRef}
          variant="outline"
          size="sm"
          onClick={() => setShowNew((value) => !value)}
        >
          <Plus className="size-3.5" /> New version
        </Button>
      </div>
      {(error || create.error || publish.error) && (
        <ErrorState
          message={
            [error, create.error, publish.error].find((item) => item instanceof Error)?.message ??
            "Version action failed"
          }
        />
      )}
      {showNew && (
        <div className="rounded-xl border border-(--color-accent)/30 bg-(--color-accent-soft)/30 p-4">
          <div className="grid gap-4 sm:grid-cols-2">
            <Field label="Semantic version" htmlFor="new-version">
              <Input id="new-version" value={version} onChange={(e) => setVersion(e.target.value)} />
            </Field>
            <Field label="Changelog" htmlFor="new-changelog">
              <Input id="new-changelog" value={changelog} onChange={(e) => setChangelog(e.target.value)} />
            </Field>
            <div className="sm:col-span-2">
              <Field label="Payload JSON" htmlFor="new-version-payload">
                <Textarea
                  id="new-version-payload"
                  value={payload}
                  onChange={(e) => setPayload(e.target.value)}
                  className="min-h-40 font-mono text-xs"
                  spellCheck={false}
                />
              </Field>
            </div>
          </div>
          <div className="mt-3 flex justify-end gap-2">
            <Button variant="ghost" size="sm" onClick={() => setShowNew(false)}>
              Cancel
            </Button>
            <Button size="sm" onClick={() => create.mutate()} disabled={create.isPending}>
              {create.isPending ? "Creating…" : "Create draft"}
            </Button>
          </div>
        </div>
      )}

      {isLoading ? (
        <SkeletonRows rows={3} />
      ) : (
        <div className="space-y-2">
          {data.map((item) => (
            <div
              key={item.id}
              className="flex flex-col gap-3 rounded-lg border border-(--border-soft) px-3 py-3 sm:flex-row sm:items-center"
            >
              <span className="grid size-8 shrink-0 place-items-center rounded-md bg-(--bg-key) text-(--color-text-muted)">
                <GitBranch className="size-4" />
              </span>
              <div className="min-w-0 flex-1">
                <div className="flex flex-wrap items-center gap-2">
                  <span className="font-mono text-sm font-medium">v{item.version}</span>
                  <VersionBadge status={item.status} />
                </div>
                <p className="mt-0.5 truncate text-xs text-(--color-text-muted)">
                  {item.changelog || "No changelog"} · {formatDate(item.created_at)}
                </p>
              </div>
              {item.status === "draft" && resource.status !== "archived" && (
                <Button variant="outline" size="sm" onClick={() => setPendingPublish(item)}>
                  Publish
                </Button>
              )}
            </div>
          ))}
        </div>
      )}
      <ConfirmDialog
        open={pendingPublish !== null}
        title={`Publish v${pendingPublish?.version ?? ""}?`}
        description="This version becomes active immediately and connected EvoFlux clients receive the update."
        confirmLabel="Publish version"
        tone="default"
        busy={publish.isPending}
        onClose={() => setPendingPublish(null)}
        onConfirm={() => pendingPublish && publish.mutate(pendingPublish.id)}
      />
    </div>
  )
}

function AccessPanel({ resource }: { resource: ManagedResource }) {
  const qc = useQueryClient()
  const { data, isLoading, error } = useQuery({
    queryKey: ["resources", resource.id, "access"],
    queryFn: () => api.resourceAccess(resource.id),
  })
  const { data: roles = [] } = useQuery({ queryKey: ["sub-roles"], queryFn: api.subRoles })
  const { data: tags = [] } = useQuery({ queryKey: ["tags"], queryFn: api.tags })
  const { data: members } = useQuery({
    queryKey: ["members", "access-options"],
    queryFn: () => api.members({ status: "active", limit: 100 }),
  })
  const [policy, setPolicy] = useState<ResourceAccessPolicy | null>(null)

  useEffect(() => {
    if (!data) return
    const noRules =
      !data.all_members &&
      data.primary_roles.length === 0 &&
      data.sub_role_ids.length === 0 &&
      data.tag_ids.length === 0 &&
      data.member_ids.length === 0
    setPolicy({
      ...data,
      all_members: data.all_members || (resource.visibility === "shared" && noRules),
    })
  }, [data, resource.visibility])

  const save = useMutation({
    mutationFn: () => api.setResourceAccess(resource.id, policy!),
    onSuccess: (next) => {
      setPolicy(next)
      void qc.invalidateQueries({ queryKey: ["resources", resource.id, "access"] })
    },
  })

  if (error) {
    return <ErrorState message={error instanceof Error ? error.message : "Access policy unavailable"} />
  }
  if (isLoading || !policy) return <SkeletonRows rows={4} />
  const disabled = policy.all_members

  return (
    <div className="space-y-5">
      {save.error && (
        <ErrorState
          message={save.error instanceof Error ? save.error.message : "Access update failed"}
        />
      )}
      <div className="rounded-xl border border-(--border-card) p-4">
        <label className="flex cursor-pointer items-start gap-3">
          <input
            type="checkbox"
            checked={policy.all_members}
            onChange={(event) => setPolicy({ ...policy, all_members: event.target.checked })}
            className="mt-0.5 size-4 accent-(--color-accent)"
          />
          <span>
            <span className="block text-sm font-medium">All active members</span>
            <span className="mt-0.5 block text-xs text-(--color-text-muted)">
              Fast default for shared resources. Turn off to target roles, teams, tags or individuals.
            </span>
          </span>
        </label>
      </div>
      <div className="grid gap-4 sm:grid-cols-2">
        <Field label="Primary roles" htmlFor="access-primary-roles">
          <MultiSelect
            id="access-primary-roles"
            disabled={disabled}
            options={[
              { value: "admin", label: "Admin" },
              { value: "contribute", label: "Contribute" },
              { value: "user", label: "User" },
            ]}
            value={policy.primary_roles}
            onChange={(primary_roles) => setPolicy({ ...policy, primary_roles })}
          />
        </Field>
        <Field label="Sub-roles" htmlFor="access-subroles">
          <MultiSelect
            id="access-subroles"
            disabled={disabled}
            options={roles.map((role) => ({ value: role.id, label: role.name }))}
            value={policy.sub_role_ids}
            onChange={(sub_role_ids) => setPolicy({ ...policy, sub_role_ids })}
          />
        </Field>
        <Field label="Member tags" htmlFor="access-tags">
          <MultiSelect
            id="access-tags"
            disabled={disabled}
            options={tags.map((tag) => ({ value: tag.id, label: tag.name }))}
            value={policy.tag_ids}
            onChange={(tag_ids) => setPolicy({ ...policy, tag_ids })}
          />
        </Field>
        <Field label="Specific members" htmlFor="access-members">
          <MultiSelect
            id="access-members"
            disabled={disabled}
            options={(members?.items ?? []).map((member) => ({
              value: member.id,
              label: `${member.display_name} · ${member.email}`,
            }))}
            value={policy.member_ids}
            onChange={(member_ids) => setPolicy({ ...policy, member_ids })}
          />
        </Field>
      </div>
      {!policy.all_members &&
        policy.primary_roles.length === 0 &&
        policy.sub_role_ids.length === 0 &&
        policy.tag_ids.length === 0 &&
        policy.member_ids.length === 0 && (
          <p className="rounded-lg border border-(--color-warning)/30 bg-(--color-warning)/8 px-3 py-2 text-xs text-(--color-warning)">
            Owner-only access: no other member will receive this resource.
          </p>
        )}
      <div className="flex justify-end">
        <Button onClick={() => save.mutate()} disabled={save.isPending}>
          <ShieldCheck className="size-3.5" />
          {save.isPending ? "Saving…" : "Save access policy"}
        </Button>
      </div>
    </div>
  )
}

function MonitoringPanel({ resource }: { resource: ManagedResource }) {
  const [days, setDays] = useState<"7" | "30" | "90">("30")
  const { data, isLoading, error } = useQuery({
    queryKey: ["resources", resource.id, "monitoring", days],
    queryFn: () => api.resourceMonitoring(resource.id, Number(days)),
  })

  if (isLoading) return <SkeletonRows rows={5} />
  if (error || !data) {
    return <ErrorState message={error instanceof Error ? error.message : "Monitoring unavailable"} />
  }

  return (
    <div className="space-y-5">
      <div className="flex items-start justify-between gap-3">
        <div>
          <h3 className="text-sm font-medium">Effectiveness monitoring</h3>
          <p className="mt-0.5 text-xs text-(--color-text-muted)">
            Usage is attributed to the member who owns each reporting secret.
          </p>
        </div>
        <Select
          value={days}
          onValueChange={setDays}
          options={[
            { value: "7", label: "7 days" },
            { value: "30", label: "30 days" },
            { value: "90", label: "90 days" },
          ]}
          className="w-28"
        />
      </div>
      <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
        <MiniMetric label="Executions" value={formatNumber(data.summary.executions)} />
        <MiniMetric label="Success rate" value={`${data.summary.success_rate}%`} />
        <MiniMetric label="Active members" value={String(data.summary.active_members)} />
        <MiniMetric
          label="Avg. rating"
          value={data.summary.average_rating ? `${data.summary.average_rating} / 5` : "No feedback"}
        />
      </div>
      <UsageChart monitoring={data} />
      <div>
        <h3 className="mb-2 text-sm font-medium">Member adoption</h3>
        {data.members.length === 0 ? (
          <EmptyState
            icon={Activity}
            title="No usage reported yet"
            description="Once EvoFlux sends usage batches, member-level outcomes appear here."
            className="py-8"
          />
        ) : (
          <TableWrap>
            <Table>
              <TableHead>
                <tr>
                  <TableTh>Member</TableTh>
                  <TableTh>Executions</TableTh>
                  <TableTh>Success</TableTh>
                  <TableTh>Avg. duration</TableTh>
                  <TableTh>Last used</TableTh>
                </tr>
              </TableHead>
              <TableBody>
                {data.members.map((member) => (
                  <TableRow key={member.user_id}>
                    <TableTd className="font-medium">{member.member_name}</TableTd>
                    <TableTd>{formatNumber(member.executions)}</TableTd>
                    <TableTd>{member.success_rate}%</TableTd>
                    <TableTd>{formatDuration(member.average_duration_ms)}</TableTd>
                    <TableTd className="text-xs text-(--color-text-muted)">
                      {formatDate(member.last_used_at)}
                    </TableTd>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </TableWrap>
        )}
      </div>
    </div>
  )
}

function UsageChart({ monitoring }: { monitoring: ResourceMonitoring }) {
  const max = Math.max(1, ...monitoring.daily.map((point) => point.executions))
  return (
    <div className="rounded-xl border border-(--border-card) p-4">
      <div className="mb-4 flex items-center justify-between gap-3">
        <div>
          <h3 className="text-sm font-medium">Daily executions</h3>
          <p className="text-xs text-(--color-text-muted)">
            {formatNumber(monitoring.summary.tokens_in + monitoring.summary.tokens_out)} tokens processed
          </p>
        </div>
        <Badge tone={monitoring.summary.success_rate >= 90 ? "success" : "warning"}>
          {monitoring.summary.success_rate}% success
        </Badge>
      </div>
      {monitoring.daily.length === 0 ? (
        <div className="grid h-44 place-items-center text-xs text-(--color-text-subtle)">
          Waiting for usage events
        </div>
      ) : (
        <div
          role="img"
          aria-label={`Daily executions for the last ${monitoring.days} days`}
          className="flex h-48 items-end gap-1 overflow-x-auto border-b border-(--border-soft) px-1"
        >
          {monitoring.daily.map((point) => (
            <div
              key={point.date}
              className="group flex h-full min-w-3 flex-1 items-end"
              title={`${point.date}: ${point.executions} executions, ${point.successes} successful`}
            >
              <div
                className="w-full rounded-t-sm bg-gradient-to-t from-(--color-accent) to-(--accent-blue) opacity-75 transition-opacity group-hover:opacity-100"
                style={{ height: `${Math.max(4, (point.executions / max) * 100)}%` }}
              />
            </div>
          ))}
        </div>
      )}
    </div>
  )
}

function FeedbackPanel({
  resource,
  canManage,
}: {
  resource: ManagedResource
  canManage: boolean
}) {
  const qc = useQueryClient()
  const [rating, setRating] = useState<"1" | "2" | "3" | "4" | "5">("5")
  const [comment, setComment] = useState("")
  const { data = [], isLoading, error } = useQuery({
    queryKey: ["resources", resource.id, "feedback"],
    queryFn: () => api.resourceFeedback(resource.id),
    enabled: canManage,
  })
  const submit = useMutation({
    mutationFn: () => api.submitResourceFeedback(resource.id, Number(rating), comment),
    onSuccess: () => {
      setComment("")
      void qc.invalidateQueries({ queryKey: ["resources", resource.id, "feedback"] })
      void qc.invalidateQueries({ queryKey: ["resources", resource.id, "monitoring"] })
    },
  })

  return (
    <div className="space-y-5">
      {(error || submit.error) && (
        <ErrorState
          message={
            error instanceof Error
              ? error.message
              : submit.error instanceof Error
                ? submit.error.message
                : "Feedback action failed"
          }
        />
      )}
      {resource.status === "published" ? (
        <div className="rounded-xl border border-(--border-card) p-4">
          <div className="mb-3 flex items-center gap-2">
            <MessageSquareText className="size-4 text-(--color-text-subtle)" />
            <div>
              <h3 className="text-sm font-medium">Your feedback on v{resource.version}</h3>
              <p className="text-xs text-(--color-text-muted)">
                One current response per member; submitting again updates it.
              </p>
            </div>
          </div>
          <div className="grid gap-4 sm:grid-cols-[9rem_1fr]">
            <Field label="Rating" htmlFor="feedback-rating">
              <Select
                id="feedback-rating"
                value={rating}
                onValueChange={setRating}
                options={[
                  { value: "5", label: "5 — Excellent" },
                  { value: "4", label: "4 — Good" },
                  { value: "3", label: "3 — Okay" },
                  { value: "2", label: "2 — Poor" },
                  { value: "1", label: "1 — Blocked" },
                ]}
              />
            </Field>
            <Field label="Comment" htmlFor="feedback-comment">
              <Textarea
                id="feedback-comment"
                value={comment}
                onChange={(e) => setComment(e.target.value)}
                placeholder="What worked, what failed, and what should change?"
              />
            </Field>
          </div>
          <div className="mt-3 flex justify-end">
            <Button size="sm" onClick={() => submit.mutate()} disabled={submit.isPending}>
              <Star className="size-3.5" />
              {submit.isPending ? "Submitting…" : "Submit feedback"}
            </Button>
          </div>
        </div>
      ) : (
        <p className="text-sm text-(--color-text-muted)">Feedback opens after the first version is published.</p>
      )}

      {canManage && (
        <div>
          <h3 className="mb-2 text-sm font-medium">Member feedback</h3>
          {isLoading ? (
            <SkeletonRows rows={3} />
          ) : data.length === 0 ? (
            <EmptyState
              icon={MessageSquareText}
              title="No feedback yet"
              description="Ask early adopters to rate the published version after real use."
              className="py-8"
            />
          ) : (
            <div className="space-y-2">
              {data.map((item) => (
                <div key={item.id} className="rounded-lg border border-(--border-soft) px-3 py-3">
                  <div className="flex flex-wrap items-center justify-between gap-2">
                    <span className="text-sm font-medium">{item.member_name}</span>
                    <span className="inline-flex items-center gap-1 text-xs text-(--color-warning)">
                      <Star className="size-3 fill-current" /> {item.rating}/5 · v{item.resource_version}
                    </span>
                  </div>
                  {item.comment && (
                    <p className="mt-2 text-sm leading-relaxed text-(--color-text-muted)">{item.comment}</p>
                  )}
                  <p className="mt-2 text-[0.7rem] text-(--color-text-subtle)">{formatDate(item.updated_at)}</p>
                </div>
              ))}
            </div>
          )}
        </div>
      )}
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

function MiniMetric({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-lg border border-(--border-soft) bg-(--bg-page) px-3 py-3">
      <div className="text-xs text-(--color-text-muted)">{label}</div>
      <div className="mt-1 truncate text-sm font-semibold capitalize">{value}</div>
    </div>
  )
}

function StatusBadge({ status }: { status: ResourceStatus }) {
  const tone = status === "published" ? "success" : status === "draft" ? "warning" : "neutral"
  return (
    <Badge tone={tone} className="capitalize">
      {status}
    </Badge>
  )
}

function VersionBadge({ status }: { status: ResourceVersion["status"] }) {
  const tone = status === "published" ? "success" : status === "draft" ? "warning" : "neutral"
  return (
    <Badge tone={tone} className="capitalize">
      {status}
    </Badge>
  )
}

function formatDate(value: string) {
  return new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" }).format(
    new Date(value),
  )
}

function formatNumber(value: number) {
  return new Intl.NumberFormat(undefined, { notation: value >= 10_000 ? "compact" : "standard" }).format(value)
}

function formatDuration(milliseconds: number) {
  if (milliseconds < 1_000) return `${milliseconds} ms`
  return `${(milliseconds / 1_000).toFixed(milliseconds < 10_000 ? 1 : 0)} s`
}

function nextPatch(version: string) {
  const parts = version.split(".").map(Number)
  if (parts.length !== 3 || parts.some(Number.isNaN)) return "0.1.0"
  return `${parts[0]}.${parts[1]}.${parts[2] + 1}`
}

function focusAfterDialogTransition(focus: () => void) {
  window.requestAnimationFrame(() => window.requestAnimationFrame(focus))
}
