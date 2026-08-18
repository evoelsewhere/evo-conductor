import { useQuery, useQueryClient } from "@tanstack/react-query"
import { useNavigate } from "@tanstack/react-router"
import {
  Activity,
  AlertTriangle,
  Boxes,
  CheckCircle2,
  LayoutGrid,
  List,
  LockKeyhole,
  Pencil,
  Plus,
  Search,
  Users,
} from "lucide-react"
import { useMemo, useState } from "react"

import { useUsageRange } from "@/features/members/components/date-range-filter"
import { ResourceUsageNav } from "@/features/resource-usage/components/resource-usage-nav"
import { PluginCreateDrawer } from "@/features/resources/components/plugin-create-drawer"
import { ResourceCreateDrawer } from "@/features/resources/components/resource-create-drawer"
import { api, type ManagedResource } from "@/shared/api/client"
import { PageFrame } from "@/shared/components/page-frame"
import {
  RESOURCE_KIND,
  RESOURCE_KIND_LABEL,
  RESOURCE_KIND_OPTIONS,
  RESOURCE_QUERY_KEY,
  RESOURCE_STATUS,
} from "@/shared/constants/resource"
import { useAuthStore } from "@/shared/stores/auth"
import { PERMISSION, mayRequest } from "@/shared/lib/authorization"
import { useMinimumLoading } from "@/shared/hooks/use-minimum-loading"
import { Badge } from "@/shared/ui/badge"
import { Button } from "@/shared/ui/button"
import { EmptyState, ErrorState } from "@/shared/ui/empty-state"
import { Input } from "@/shared/ui/input"
import { Select } from "@/shared/ui/select"
import { LoadingState, Skeleton } from "@/shared/ui/skeleton"
import { cn } from "@/shared/lib/utils"
import {
  Table,
  TableBody,
  TableHead,
  TableRow,
  TableTd,
  TableTh,
  TableWrap,
} from "@/shared/ui/table"

type ResourceKind = ManagedResource["kind"]
type ResourceStatus = ManagedResource["status"]
type CatalogView = "grid" | "table"
type CatalogUsage = {
  uses: number
  requests: number
  successes: number
  errors: number
  members: number
}

const kindOptions = [
  { value: "all", label: "All types" },
  ...RESOURCE_KIND_OPTIONS,
] as const

const kindPageMeta: Partial<Record<ResourceKind, { title: string; subtitle: string }>> = {
  [RESOURCE_KIND.PLUGIN]: {
    title: "Plugins",
    subtitle: "Portable Plugins published to EvoFlux clients, from Draft to measurable outcomes.",
  },
  [RESOURCE_KIND.SKILL]: {
    title: "Skills",
    subtitle: "Govern reusable skills from draft to measurable outcomes.",
  },
  [RESOURCE_KIND.AGENT]: {
    title: "Agents",
    subtitle: "Govern agent definitions from draft to measurable outcomes.",
  },
}

const statusOptions = [
  { value: "all", label: "All status" },
  { value: RESOURCE_STATUS.PUBLISHED, label: "Published" },
  { value: RESOURCE_STATUS.BETA, label: "Beta" },
  { value: RESOURCE_STATUS.DRAFT, label: "Draft" },
  { value: RESOURCE_STATUS.ARCHIVED, label: "Archived" },
] as const

export function ResourcesPage({ fixedKind }: { fixedKind?: ResourceKind }) {
  const queryClient = useQueryClient()
  const navigate = useNavigate()
  const actor = useAuthStore((state) => state.user)
  const can = useAuthStore((state) => state.can)
  const canCreate = mayRequest(
    can(PERMISSION.RESOURCE_AUTHOR, { ownerId: actor?.id }),
  )
  const canMonitor = mayRequest(can(PERMISSION.TELEMETRY_PROJECT_READ))
  const catalogDates = useUsageRange()
  const resources = useQuery({
    queryKey: [RESOURCE_QUERY_KEY],
    queryFn: () => api.resources(),
  })
  const kindUsage = useQuery({
    queryKey: ["resource-catalog-monitoring", fixedKind, catalogDates.range],
    queryFn: () =>
      api.resourceUsage({
        ...catalogDates.range,
        resource_kind: fixedKind,
        limit: 8,
      }),
    enabled: Boolean(fixedKind && canMonitor),
  })
  const [query, setQuery] = useState("")
  const [kind, setKind] = useState<(typeof kindOptions)[number]["value"]>(fixedKind ?? "all")
  const [status, setStatus] = useState<(typeof statusOptions)[number]["value"]>("all")
  const [showCreate, setShowCreate] = useState(false)
  const [catalogView, setCatalogView] = useState<CatalogView>(() =>
    window.localStorage.getItem(`conductor.catalog-view.${fixedKind ?? "all"}`) === "table"
      ? "table"
      : "grid",
  )

  const scopedResources = useMemo(
    () =>
      fixedKind === undefined
        ? (resources.data ?? [])
        : (resources.data ?? []).filter((resource) => resource.kind === fixedKind),
    [fixedKind, resources.data],
  )

  const filtered = useMemo(() => {
    const term = query.trim().toLowerCase()
    return scopedResources.filter(
      (resource) =>
        (fixedKind !== undefined || kind === "all" || resource.kind === kind) &&
        (status === "all" || resource.status === status) &&
        (!term ||
          resource.name.toLowerCase().includes(term) ||
          resource.slug.toLowerCase().includes(term)),
    )
  }, [fixedKind, kind, query, scopedResources, status])

  const published = scopedResources.filter(
    (resource) => resource.status === RESOURCE_STATUS.PUBLISHED,
  ).length
  const drafts = scopedResources.filter(
    (resource) => resource.status === RESOURCE_STATUS.DRAFT,
  ).length
  const catalogKinds = new Set(scopedResources.map((resource) => resource.kind)).size
  const usageByResource = useMemo(() => {
    const result = new Map<string, {
      uses: number
      requests: number
      successes: number
      errors: number
      members: number
    }>()
    for (const item of kindUsage.data?.resources ?? []) {
      const current = result.get(item.resource_id) ?? {
        uses: 0,
        requests: 0,
        successes: 0,
        errors: 0,
        members: 0,
      }
      current.uses += item.uses
      current.requests += item.requests
      current.successes += item.successes
      current.errors += item.errors
      current.members = Math.max(current.members, item.members)
      result.set(item.resource_id, current)
    }
    return result
  }, [kindUsage.data?.resources])
  const completedRequests =
    (kindUsage.data?.totals.successes ?? 0) + (kindUsage.data?.totals.errors ?? 0)
  const successRate = completedRequests
    ? Math.round(((kindUsage.data?.totals.successes ?? 0) / completedRequests) * 100)
    : null
  const resourcesInitialLoading = useMinimumLoading(
    resources.isLoading && !resources.data,
  )
  const usageInitialLoading = useMinimumLoading(
    Boolean(fixedKind && canMonitor && kindUsage.isLoading && !kindUsage.data),
  )
  const resourcesFatal = Boolean(
    resources.error && !resources.data && !resourcesInitialLoading,
  )
  const usageFatal = Boolean(
    kindUsage.error && !kindUsage.data && !usageInitialLoading,
  )

  function canManage(resource: ManagedResource) {
    return mayRequest(
      can(PERMISSION.RESOURCE_AUTHOR, {
        ownerId: resource.owner_user_id,
        resourceKind: resource.kind,
        lifecycle: resource.status,
      }),
    )
  }

  function handleCreated(resource: ManagedResource) {
    setShowCreate(false)
    void queryClient.invalidateQueries({ queryKey: [RESOURCE_QUERY_KEY] })
    void navigate({
      to: "/app/resources/$kind/$resourceId/edit",
      params: { kind: resource.kind, resourceId: resource.id },
    })
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
      className="max-w-7xl"
      action={
        canCreate ? (
          <Button variant="gradient" onClick={() => setShowCreate(true)}>
            <Plus className="size-3.5" />
            {fixedKind
              ? `Add ${RESOURCE_KIND_LABEL[fixedKind].toLowerCase()}`
              : "New resource"}
          </Button>
        ) : undefined
      }
    >
      {fixedKind && canMonitor && (
        <ResourceUsageNav
          kind={fixedKind as Extract<ResourceKind, "plugin" | "skill" | "agent">}
        />
      )}
      <span className="sr-only" role="status" aria-live="polite" aria-atomic="true">
        {resourcesInitialLoading
          ? "Loading resource catalog…"
          : usageInitialLoading
            ? "Loading resource usage…"
            : ""}
      </span>

      {!resourcesFatal && !usageFatal && (
        resourcesInitialLoading || usageInitialLoading ? (
          <CatalogSummarySkeleton count={fixedKind && canMonitor ? 4 : 3} announce={false} />
        ) : (
          <CatalogSummary
            items={
          fixedKind && canMonitor && kindUsage.data
            ? [
                {
                  label: "Published",
                  value: published,
                  hint: `${drafts} drafts · ${scopedResources.length} total`,
                  icon: CheckCircle2,
                  tone: "success",
                },
                {
                  label: "Delivery",
                  value: kindUsage.data?.totals.installed_installations ?? 0,
                  hint: `${kindUsage.data?.totals.pending_installations ?? 0} pending · ${kindUsage.data?.totals.attention_installations ?? 0} need attention`,
                  icon: Activity,
                  tone: (kindUsage.data?.totals.attention_installations ?? 0) > 0 ? "warning" : "success",
                },
                {
                  label: `Requests · ${catalogDates.preset}`,
                  value: kindUsage.data?.totals.requests ?? 0,
                  hint: `${kindUsage.data?.totals.resource_uses ?? 0} attributed resource uses`,
                  icon: Users,
                  tone: "accent",
                },
                {
                  label: "Reliability",
                  value: successRate == null ? "—" : `${successRate}%`,
                  hint: `${kindUsage.data?.totals.errors ?? 0} errors · ${kindUsage.data?.totals.blocked ?? 0} blocked`,
                  icon: AlertTriangle,
                  tone: successRate != null && successRate < 90 ? "warning" : "success",
                },
              ]
            : fixedKind
              ? [
                  { label: "Published", value: published, hint: "Available releases", icon: CheckCircle2, tone: "success" },
                  { label: "Drafts", value: drafts, hint: "Work in progress", icon: Pencil, tone: "warning" },
                  { label: "Total", value: scopedResources.length, hint: "Accessible resources", icon: Boxes, tone: "accent" },
                ]
              : [
                  { label: "Published", value: published, hint: "Available releases", icon: CheckCircle2, tone: "success" },
                  { label: "Drafts", value: drafts, hint: "Work in progress", icon: Pencil, tone: "warning" },
                  { label: "Resource types", value: catalogKinds, hint: `${scopedResources.length} resources total`, icon: Boxes, tone: "accent" },
                ]
            }
          />
        )
      )}

      {fixedKind && canMonitor && !usageInitialLoading && kindUsage.isSuccess && (kindUsage.data.totals.requests ?? 0) === 0 && (
        <div className="mb-4 flex flex-col gap-3 rounded-xl border border-(--border-card) bg-(--bg-card) px-4 py-3 sm:flex-row sm:items-center sm:justify-between">
          <div className="flex items-start gap-2.5">
            <AlertTriangle className="mt-0.5 size-4 shrink-0 text-(--color-warning)" />
            <div>
              <div className="text-xs font-medium">Waiting for attributed EvoFlux telemetry</div>
              <p className="mt-0.5 text-[0.7rem] text-(--color-text-muted)">
                Manage the catalog now; charts and reports appear in Analytics Studio after the first managed request.
              </p>
            </div>
          </div>
          <Button
            variant="outline"
            size="sm"
            onClick={() =>
              void navigate({
                to: fixedKind === RESOURCE_KIND.PLUGIN
                  ? "/app/resources/plugins/usage"
                  : fixedKind === RESOURCE_KIND.SKILL
                    ? "/app/resources/skills/usage"
                    : "/app/resources/agents/usage",
              })
            }
          >
            Open Analytics Studio
          </Button>
        </div>
      )}

      <div className="mb-4 rounded-xl border border-(--border-card) bg-(--bg-card) p-3">
        <div className="flex flex-col gap-2 lg:flex-row lg:items-center">
          <div className="relative min-w-0 flex-1">
            <Search className="pointer-events-none absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2 text-(--color-text-subtle)" />
            <Input
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              placeholder="Search by name or slug…"
              aria-label="Search resources"
              className="pl-8"
            />
          </div>
          {fixedKind === undefined && (
            <Select
              value={kind}
              onValueChange={setKind}
              options={kindOptions}
              className="lg:w-40"
            />
          )}
          <Select
            value={status}
            onValueChange={setStatus}
            options={statusOptions}
            className="lg:w-40"
          />
          <div className="flex rounded-md border border-(--color-border) bg-(--bg-page) p-0.5" aria-label="Catalog layout">
            {(["grid", "table"] as CatalogView[]).map((view) => {
              const Icon = view === "grid" ? LayoutGrid : List
              return (
                <button
                  key={view}
                  type="button"
                  aria-label={`${view} view`}
                  aria-pressed={catalogView === view}
                  onClick={() => {
                    setCatalogView(view)
                    window.localStorage.setItem(`conductor.catalog-view.${fixedKind ?? "all"}`, view)
                  }}
                  className={cn(
                    "grid size-8 place-items-center rounded-sm transition-colors",
                    catalogView === view
                      ? "bg-(--bg-key) text-(--color-text) shadow-sm"
                      : "text-(--color-text-subtle) hover:text-(--color-text)",
                  )}
                >
                  <Icon className="size-3.5" />
                </button>
              )
            })}
          </div>
        </div>
      </div>

      {(resources.error || kindUsage.error) &&
        !resourcesInitialLoading &&
        !usageInitialLoading && (
        <ErrorState
          className="mb-4"
          message={
            resources.error instanceof Error
              ? resources.error.message
              : kindUsage.error instanceof Error
                ? kindUsage.error.message
                : "Catalog data failed to load"
          }
        />
      )}

      {resourcesInitialLoading ? (
        catalogView === "grid" ? (
          <ResourceCatalogGridSkeleton announce={false} />
        ) : (
          <ResourceCatalogTableSkeleton canMonitor={canMonitor} announce={false} />
        )
      ) : resourcesFatal ? null : filtered.length === 0 ? (
        <EmptyState
          icon={Boxes}
          title={
            scopedResources.length === 0
              ? fixedKind === undefined
                ? "Build your governed catalog"
                : `No ${meta.title.toLowerCase()} yet`
              : "No matching resources"
          }
          description={
            scopedResources.length === 0
              ? "Create a draft, define access, publish a version, then monitor how members use it."
              : "Try another search, type, or status filter."
          }
          action={
            scopedResources.length === 0 && canCreate ? (
              <Button variant="outline" onClick={() => setShowCreate(true)}>
                {fixedKind === undefined
                  ? "Create first resource"
                  : `Create first ${RESOURCE_KIND_LABEL[fixedKind].toLowerCase()}`}
              </Button>
            ) : undefined
          }
        />
      ) : catalogView === "grid" ? (
        <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-3">
          {filtered.map((resource) => (
            <ResourceCatalogCard
              key={resource.id}
              resource={resource}
              usage={usageByResource.get(resource.id)}
              usageLoading={usageInitialLoading}
              usageUnavailable={Boolean(kindUsage.error)}
              canManage={canManage(resource)}
              onOpen={() =>
                void navigate({
                  to: "/app/resources/$kind/$resourceId",
                  params: { kind: resource.kind, resourceId: resource.id },
                })
              }
            />
          ))}
        </div>
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
                {canMonitor && <TableTh>Usage</TableTh>}
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
                    <Badge>{RESOURCE_KIND_LABEL[resource.kind]}</Badge>
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
                  {canMonitor && (
                    <TableTd>
                      <ResourceUsageCell
                        usage={usageByResource.get(resource.id)}
                        loading={usageInitialLoading}
                        unavailable={Boolean(kindUsage.error)}
                      />
                    </TableTd>
                  )}
                  <TableTd className="text-xs text-(--color-text-muted)">
                    {formatDate(resource.updated_at)}
                  </TableTd>
                  <TableTd className="text-right">
                    <Button
                      variant="ghost"
                      size="sm"
                      aria-label={`${canManage(resource) ? "Manage" : "View"} ${resource.name}`}
                      onClick={() =>
                        void navigate({
                          to: "/app/resources/$kind/$resourceId",
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

      {fixedKind === RESOURCE_KIND.PLUGIN ? (
        <PluginCreateDrawer
          open={showCreate && canCreate}
          onClose={() => setShowCreate(false)}
          onCreated={handleCreated}
        />
      ) : (
        <ResourceCreateDrawer
          open={showCreate && canCreate}
          defaultKind={fixedKind}
          onClose={() => setShowCreate(false)}
          onCreated={handleCreated}
        />
      )}
    </PageFrame>
  )
}

function CatalogSummary({
  items,
}: {
  items: Array<{
    label: string
    value: number | string
    hint: string
    icon: typeof Boxes
    tone: "success" | "warning" | "accent"
  }>
}) {
  const colors = {
    success: "text-(--color-success) bg-(--color-success)/10",
    warning: "text-(--color-warning) bg-(--color-warning)/10",
    accent: "text-(--color-accent) bg-(--color-accent-soft)",
  }
  return (
    <div className={cn("mb-4 grid overflow-hidden rounded-xl border border-(--border-card) bg-(--bg-card) sm:grid-cols-2", items.length >= 4 ? "xl:grid-cols-4" : "xl:grid-cols-3")}>
      {items.map(({ label, value, hint, icon: Icon, tone }, index) => (
        <div key={label} className={cn("flex min-w-0 items-start gap-3 border-b border-(--border-soft) p-4 sm:border-r", index >= items.length - 2 && "sm:border-b-0", index === items.length - 1 && "border-b-0 sm:border-r-0")}>
          <span className={`grid size-8 shrink-0 place-items-center rounded-lg ${colors[tone]}`}>
            <Icon className="size-4" />
          </span>
          <div className="min-w-0">
            <div className="text-xs font-medium text-(--color-text-muted)">{label}</div>
            <div className="mt-1 text-xl font-semibold tabular-nums">
              {typeof value === "number" ? value.toLocaleString() : value}
            </div>
            <p className="mt-0.5 text-[0.68rem] leading-relaxed text-(--color-text-subtle)">{hint}</p>
          </div>
        </div>
      ))}
    </div>
  )
}

function CatalogSummarySkeleton({ count, announce = true }: { count: number; announce?: boolean }) {
  return (
    <LoadingState
      label="Loading catalog summary"
      announce={announce}
      className={cn(
        "mb-4 grid overflow-hidden rounded-xl border border-(--border-card) bg-(--bg-card) sm:grid-cols-2",
        count >= 4 ? "xl:grid-cols-4" : "xl:grid-cols-3",
      )}
    >
      {Array.from({ length: count }, (_, index) => (
        <div
          key={index}
          className="flex min-w-0 items-start gap-3 border-b border-(--border-soft) p-4 sm:border-r"
        >
          <Skeleton className="size-8 shrink-0 rounded-lg" />
          <div className="min-w-0 flex-1 space-y-2">
            <Skeleton className="h-3 w-20" />
            <Skeleton className="h-6 w-14" />
            <Skeleton className="h-2.5 w-28 max-w-full" />
          </div>
        </div>
      ))}
    </LoadingState>
  )
}

function ResourceCatalogGridSkeleton({ announce = true }: { announce?: boolean }) {
  return (
    <LoadingState
      label="Loading resource catalog"
      announce={announce}
      className="grid gap-3 md:grid-cols-2 xl:grid-cols-3"
    >
      {Array.from({ length: 6 }, (_, index) => (
        <article
          key={index}
          className="flex min-h-64 flex-col rounded-xl border border-(--border-card) bg-(--bg-card) p-4"
        >
          <div className="flex items-start justify-between gap-3">
            <Skeleton className="size-9 rounded-lg" />
            <div className="flex gap-1.5">
              <Skeleton className="h-5 w-14 rounded-full" />
              <Skeleton className="h-5 w-16 rounded-full" />
            </div>
          </div>
          <div className="mt-3 space-y-2">
            <Skeleton className="h-4 w-2/3" />
            <Skeleton className="h-2.5 w-1/3" />
            <Skeleton className="mt-3 h-3 w-full" />
            <Skeleton className="h-3 w-4/5" />
          </div>
          <div className="mt-4 grid grid-cols-2 gap-x-4 gap-y-3 border-y border-(--border-soft) py-3">
            {Array.from({ length: 4 }, (_, metric) => (
              <div key={metric} className="space-y-1.5">
                <Skeleton className="h-2.5 w-16" />
                <Skeleton className="h-3 w-12" />
              </div>
            ))}
          </div>
          <div className="mt-auto flex items-center justify-between pt-3">
            <Skeleton className="h-3 w-28" />
            <Skeleton className="h-7 w-16" />
          </div>
        </article>
      ))}
    </LoadingState>
  )
}

function ResourceCatalogTableSkeleton({
  canMonitor,
  announce = true,
}: {
  canMonitor: boolean
  announce?: boolean
}) {
  return (
    <LoadingState label="Loading resource catalog" announce={announce}>
      <TableWrap>
        <Table>
          <TableHead>
            <tr>
              <TableTh>Resource</TableTh>
              <TableTh>Type</TableTh>
              <TableTh>Status</TableTh>
              <TableTh>Version</TableTh>
              <TableTh>Access</TableTh>
              {canMonitor && <TableTh>Usage</TableTh>}
              <TableTh>Updated</TableTh>
              <TableTh />
            </tr>
          </TableHead>
          <TableBody>
            {Array.from({ length: 5 }, (_, index) => (
              <TableRow key={index}>
                <TableTd><Skeleton className="h-8 w-36" /></TableTd>
                <TableTd><Skeleton className="h-5 w-14 rounded-full" /></TableTd>
                <TableTd><Skeleton className="h-5 w-16 rounded-full" /></TableTd>
                <TableTd><Skeleton className="h-3 w-12" /></TableTd>
                <TableTd><Skeleton className="h-3 w-16" /></TableTd>
                {canMonitor && <TableTd><Skeleton className="h-8 w-24" /></TableTd>}
                <TableTd><Skeleton className="h-3 w-24" /></TableTd>
                <TableTd><Skeleton className="ml-auto size-7" /></TableTd>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </TableWrap>
    </LoadingState>
  )
}

function ResourceCatalogCard({
  resource,
  usage,
  usageLoading,
  usageUnavailable,
  canManage,
  onOpen,
}: {
  resource: ManagedResource
  usage?: CatalogUsage
  usageLoading: boolean
  usageUnavailable: boolean
  canManage: boolean
  onOpen: () => void
}) {
  const completed = (usage?.successes ?? 0) + (usage?.errors ?? 0)
  const reliability = completed
    ? `${Math.round(((usage?.successes ?? 0) / completed) * 100)}%`
    : "—"
  return (
    <article className="group flex min-h-64 flex-col rounded-xl border border-(--border-card) bg-(--bg-card) p-4 transition-colors hover:border-(--color-border-strong)">
      <div className="flex items-start justify-between gap-3">
        <span className="grid size-9 shrink-0 place-items-center rounded-lg bg-(--color-accent-soft) text-(--color-accent)">
          <Boxes className="size-4" />
        </span>
        <div className="flex flex-wrap justify-end gap-1.5">
          <Badge>{RESOURCE_KIND_LABEL[resource.kind]}</Badge>
          <StatusBadge status={resource.status} />
        </div>
      </div>
      <div className="mt-3 min-w-0">
        <h2 className="truncate text-sm font-semibold" title={resource.name}>{resource.name}</h2>
        <p className="mt-0.5 truncate font-mono text-[0.68rem] text-(--color-text-subtle)">{resource.slug}</p>
        <p className="mt-2 line-clamp-2 min-h-9 text-xs leading-relaxed text-(--color-text-muted)">
          {resource.description || `Governed ${RESOURCE_KIND_LABEL[resource.kind].toLowerCase()} ready for source, access and release management.`}
        </p>
      </div>
      <dl className="mt-4 grid grid-cols-2 gap-x-4 gap-y-3 border-y border-(--border-soft) py-3 text-xs">
        <DefinitionTerm label="Active version" value={`v${resource.version}`} />
        <DefinitionTerm label="Channel" value={resource.release_channel ?? "Draft only"} capitalize />
        {usageLoading ? (
          <CatalogUsageMetricSkeleton label="Uses" />
        ) : (
          <DefinitionTerm
            label="Uses"
            value={usageUnavailable ? "Unavailable" : usage ? usage.uses.toLocaleString() : "—"}
          />
        )}
        {usageLoading ? (
          <CatalogUsageMetricSkeleton label="Reliability" />
        ) : (
          <DefinitionTerm
            label="Reliability"
            value={usageUnavailable ? "Unavailable" : reliability}
          />
        )}
      </dl>
      <div className="mt-auto flex items-center justify-between gap-3 pt-3">
        <span className="inline-flex min-w-0 items-center gap-1 text-[0.68rem] capitalize text-(--color-text-subtle)">
          {resource.visibility === "private" ? <LockKeyhole className="size-3" /> : <Users className="size-3" />}
          {resource.visibility} · {formatRelativeDate(resource.updated_at)}
        </span>
        <Button variant="ghost" size="sm" onClick={onOpen}>
          {canManage ? "Manage" : "View"}
        </Button>
      </div>
    </article>
  )
}

function CatalogUsageMetricSkeleton({ label }: { label: string }) {
  return (
    <div className="min-w-0">
      <dt className="text-[0.65rem] text-(--color-text-subtle)">{label}</dt>
      <dd className="mt-1.5">
        <span className="sr-only">Loading</span>
        <Skeleton className="h-3 w-12" />
      </dd>
    </div>
  )
}

function DefinitionTerm({
  label,
  value,
  capitalize = false,
}: {
  label: string
  value: string
  capitalize?: boolean
}) {
  return (
    <div className="min-w-0">
      <dt className="text-[0.65rem] text-(--color-text-subtle)">{label}</dt>
      <dd className={cn("mt-0.5 truncate font-medium", capitalize && "capitalize")}>{value}</dd>
    </div>
  )
}

function ResourceUsageCell({
  usage,
  loading,
  unavailable,
}: {
  usage?: CatalogUsage
  loading: boolean
  unavailable: boolean
}) {
  if (loading) {
    return (
      <div className="space-y-1.5">
        <span className="sr-only">Loading resource usage</span>
        <Skeleton className="h-3 w-20" />
        <Skeleton className="h-2.5 w-28" />
      </div>
    )
  }
  if (unavailable) {
    return <span className="text-xs text-(--color-text-subtle)">Unavailable</span>
  }
  if (!usage) return <span className="text-xs text-(--color-text-subtle)">Awaiting signal</span>
  const completed = usage.successes + usage.errors
  const rate = completed ? Math.round((usage.successes / completed) * 100) : null
  return (
    <div className="text-xs">
      <div className="font-medium tabular-nums">{usage.uses.toLocaleString()} uses</div>
      <div className="mt-0.5 text-(--color-text-subtle)">
        {usage.members} members · {rate == null ? "—" : `${rate}%`} reliable
      </div>
    </div>
  )
}

function StatusBadge({ status }: { status: ResourceStatus }) {
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

function formatDate(value: string) {
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(new Date(value))
}

function formatRelativeDate(value: string) {
  const elapsed = Date.now() - new Date(value).getTime()
  const days = Math.max(0, Math.floor(elapsed / 86_400_000))
  if (days === 0) return "updated today"
  if (days === 1) return "updated yesterday"
  if (days < 30) return `updated ${days}d ago`
  return formatDate(value)
}
