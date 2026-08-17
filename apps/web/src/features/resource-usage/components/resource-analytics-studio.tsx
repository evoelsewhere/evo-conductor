import {
  Area,
  AreaChart,
  Bar,
  BarChart,
  CartesianGrid,
  Cell,
  Line,
  LineChart,
  Pie,
  PieChart,
  XAxis,
  YAxis,
} from "recharts"
import {
  ArrowDown,
  ArrowUp,
  BarChart3,
  CheckCircle2,
  Columns2,
  Download,
  FileJson,
  FileSpreadsheet,
  Gauge,
  LayoutDashboard,
  Maximize2,
  Plus,
  RotateCcw,
  Save,
  Settings2,
  Sparkles,
  X,
} from "lucide-react"
import { useEffect, useState } from "react"
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"

import { formatShortDate, formatTokens } from "@/features/members/components/usage-formatters"
import { formatEstimatedCost } from "@/features/resource-usage/components/resource-usage-formatters"
import { exportResourceAnalytics } from "@/features/resource-usage/lib/resource-analytics-export"
import type {
  AnalyticsView,
  AnalyticsViewDefinition,
  AnalyticsViewVisibility,
  ResourceUsageAnalytics,
  ResourceUsageDay,
} from "@/shared/api/client"
import { api } from "@/shared/api/client"
import type { ResourceKind } from "@/shared/constants/resource"
import { TELEMETRY_CHART_SERIES } from "@/shared/constants/telemetry"
import {
  PERMISSION,
  bestAuthorizationDecision,
  mayRequest,
} from "@/shared/lib/authorization"
import { cn } from "@/shared/lib/utils"
import { useAuthStore } from "@/shared/stores/auth"
import { Badge } from "@/shared/ui/badge"
import { Button } from "@/shared/ui/button"
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/shared/ui/card"
import { Dialog } from "@/shared/ui/dialog"
import { Input } from "@/shared/ui/input"
import { Label } from "@/shared/ui/label"
import {
  AccessibleChartTable,
  ChartContainer,
  ChartLegendList,
  ChartTooltip,
  ChartTooltipContent,
  type ChartConfig,
} from "@/shared/ui/chart"
import {
  Menu,
  MenuGroup,
  MenuGroupLabel,
  MenuItem,
  MenuSeparator,
} from "@/shared/ui/menu"
import { Select } from "@/shared/ui/select"

type DashboardPreset = "executive" | "adoption" | "reliability" | "cost" | "custom"
type DashboardDensity = "comfortable" | "compact"
type WidgetWidth = "half" | "full"
type WidgetView = "area" | "line" | "bar" | "donut" | "table"
type WidgetId =
  | "requests"
  | "outcomes"
  | "consumption"
  | "resources"
  | "members"
  | "models"
  | "tools"
  | "roles"

interface WidgetState {
  id: WidgetId
  width: WidgetWidth
  view: WidgetView
}

interface DashboardState {
  preset: DashboardPreset
  density: DashboardDensity
  widgets: WidgetState[]
}

const PRESET_OPTIONS = [
  { value: "executive", label: "Executive overview" },
  { value: "adoption", label: "Adoption & delivery" },
  { value: "reliability", label: "Reliability" },
  { value: "cost", label: "Cost & models" },
  { value: "custom", label: "My dashboard" },
] as const

const WIDGET_META: Record<
  WidgetId,
  {
    title: string
    description: string
    views: WidgetView[]
    defaultView: WidgetView
  }
> = {
  requests: {
    title: "Request volume",
    description: "Daily traffic and successful completions.",
    views: ["area", "line", "bar", "table"],
    defaultView: "area",
  },
  outcomes: {
    title: "Outcome distribution",
    description: "Success, error, blocked and cancelled requests over time.",
    views: ["area", "line", "bar", "table"],
    defaultView: "area",
  },
  consumption: {
    title: "Tokens & estimated cost",
    description: "Daily token volume with the priced model-call trend.",
    views: ["area", "line", "bar", "table"],
    defaultView: "area",
  },
  resources: {
    title: "Resource adoption",
    description: "Most-used immutable resource versions.",
    views: ["bar", "donut", "table"],
    defaultView: "donut",
  },
  members: {
    title: "Member adoption",
    description: "Members with the most attributed requests.",
    views: ["bar", "donut", "table"],
    defaultView: "bar",
  },
  models: {
    title: "Model calls",
    description: "Provider and model activity while governed resources were active.",
    views: ["bar", "donut", "table"],
    defaultView: "bar",
  },
  tools: {
    title: "Tool calls",
    description: "Tools invoked most often, including reported failures.",
    views: ["bar", "donut", "table"],
    defaultView: "bar",
  },
  roles: {
    title: "Usage by role",
    description: "Model and tool calls grouped by the role captured at ingest.",
    views: ["bar", "donut", "table"],
    defaultView: "bar",
  },
}

const PRESET_WIDGETS: Record<Exclude<DashboardPreset, "custom">, WidgetState[]> = {
  executive: [
    { id: "requests", width: "full", view: "area" },
    { id: "outcomes", width: "half", view: "bar" },
    { id: "resources", width: "half", view: "donut" },
    { id: "consumption", width: "full", view: "area" },
    { id: "members", width: "half", view: "bar" },
    { id: "tools", width: "half", view: "bar" },
  ],
  adoption: [
    { id: "requests", width: "full", view: "area" },
    { id: "resources", width: "half", view: "donut" },
    { id: "members", width: "half", view: "bar" },
    { id: "roles", width: "half", view: "bar" },
    { id: "models", width: "half", view: "bar" },
  ],
  reliability: [
    { id: "outcomes", width: "full", view: "area" },
    { id: "tools", width: "half", view: "bar" },
    { id: "requests", width: "half", view: "line" },
    { id: "resources", width: "full", view: "table" },
  ],
  cost: [
    { id: "consumption", width: "full", view: "area" },
    { id: "models", width: "half", view: "bar" },
    { id: "roles", width: "half", view: "bar" },
    { id: "resources", width: "full", view: "table" },
  ],
}

const OUTCOME_CONFIG = {
  successes: { label: "Success", color: "var(--color-success)" },
  errors: { label: "Error", color: "var(--color-danger)" },
  blocked: { label: "Blocked", color: "var(--color-warning)" },
  cancelled: { label: "Cancelled", color: "var(--color-text-subtle)" },
} satisfies ChartConfig

const REQUEST_CONFIG = {
  requests: { label: "Requests", color: "var(--chart-series-1)" },
  successes: { label: "Successful", color: "var(--color-success)" },
} satisfies ChartConfig

const CONSUMPTION_CONFIG = {
  total_tokens: { label: "Tokens", color: "var(--chart-series-2)" },
  estimated_cost_usd_micros: {
    label: "Estimated cost",
    color: "var(--chart-series-5)",
  },
} satisfies ChartConfig

export function ResourceAnalyticsStudio({
  data,
  loading,
  scopeLabel,
  storageKey,
  scope,
  query,
  onApplyQuery,
  allowMemberDetail = true,
}: {
  data?: ResourceUsageAnalytics
  loading: boolean
  scopeLabel: string
  storageKey: string
  scope?: { resourceKind?: ResourceKind; resourceId?: string }
  query?: AnalyticsViewDefinition["query"]
  onApplyQuery?: (query: AnalyticsViewDefinition["query"]) => void
  allowMemberDetail?: boolean
}) {
  const queryClient = useQueryClient()
  const actorId = useAuthStore((state) => state.user?.id)
  const authorization = useAuthStore((state) => state.authorization)
  const can = useAuthStore((state) => state.can)
  const [customizing, setCustomizing] = useState(false)
  const [savedViewId, setSavedViewId] = useState<string | null>(null)
  const [showSaveView, setShowSaveView] = useState(false)
  const [viewName, setViewName] = useState("")
  const [viewVisibility, setViewVisibility] = useState<AnalyticsViewVisibility>("private")
  const [dashboard, setDashboard] = useState<DashboardState>(() =>
    dashboardForAccess(
      readDashboard(storageKey) ?? defaultDashboard("executive"),
      allowMemberDetail,
    ),
  )
  const canCreateView = mayRequest(
    can(PERMISSION.ANALYTICS_VIEW_MANAGE_SELF, { ownerId: actorId }),
  )
  const canReadViews = mayRequest(can(PERMISSION.ANALYTICS_VIEW_READ))
  const savedViewsQueryKey = [
    "analytics-views",
    actorId,
    authorization?.current_role,
    authorization?.policy_revision,
  ] as const
  const savedViews = useQuery({
    queryKey: savedViewsQueryKey,
    queryFn: api.analyticsViews,
    enabled: canReadViews,
  })
  const visibleSavedViews = (savedViews.data ?? []).filter(
    (view) =>
      savedViewMatchesScope(view, scope) &&
      (allowMemberDetail ||
        (!view.definition.query.member_id && !view.definition.query.installation_id)),
  )
  const selectedSavedView = visibleSavedViews.find((view) => view.id === savedViewId)
  const canManageSelectedView = selectedSavedView
    ? mayRequest(
        bestAuthorizationDecision([
          can(PERMISSION.ANALYTICS_VIEW_MANAGE_SELF, {
            ownerId: selectedSavedView.owner_user_id,
          }),
          can(PERMISSION.ANALYTICS_VIEW_MANAGE_ANY, {
            ownerId: selectedSavedView.owner_user_id,
          }),
        ]),
      )
    : canCreateView
  const createView = useMutation({
    mutationFn: () =>
      api.createAnalyticsView({
        name: viewName.trim(),
        description: `${scopeLabel} dashboard created in Analytics Studio`,
        visibility: viewVisibility,
        definition: dashboardDefinition(
          dashboardForAccess(dashboard, allowMemberDetail),
          sanitizeAnalyticsQuery(query, allowMemberDetail),
          scope,
        ),
      }),
    onSuccess: (view) => {
      setSavedViewId(view.id)
      setShowSaveView(false)
      void queryClient.invalidateQueries({ queryKey: savedViewsQueryKey })
    },
  })
  const updateView = useMutation({
    mutationFn: () => {
      if (!selectedSavedView) throw new Error("Select a saved view before updating it.")
      return api.updateAnalyticsView(selectedSavedView.id, {
        name: selectedSavedView.name,
        description: selectedSavedView.description,
        visibility: selectedSavedView.visibility,
        definition: dashboardDefinition(
          dashboardForAccess(dashboard, allowMemberDetail),
          sanitizeAnalyticsQuery(query, allowMemberDetail),
          scope,
        ),
        revision: selectedSavedView.revision,
      })
    },
    onSuccess: (view) => {
      queryClient.setQueryData<AnalyticsView[]>(savedViewsQueryKey, (current = []) =>
        current.map((item) => (item.id === view.id ? view : item)),
      )
    },
  })
  const deleteView = useMutation({
    mutationFn: () => {
      if (!selectedSavedView) throw new Error("Select a saved view before deleting it.")
      return api.deleteAnalyticsView(selectedSavedView.id, selectedSavedView.revision)
    },
    onSuccess: () => {
      setSavedViewId(null)
      setDashboard(
        dashboardForAccess(defaultDashboard("executive"), allowMemberDetail),
      )
      void queryClient.invalidateQueries({ queryKey: savedViewsQueryKey })
    },
  })
  const visibleData = analyticsDataForAccess(data, allowMemberDetail)

  useEffect(() => {
    const safeDashboard = dashboardForAccess(dashboard, allowMemberDetail)
    window.localStorage.setItem(storageKey, JSON.stringify(safeDashboard))
    if (safeDashboard.widgets.length !== dashboard.widgets.length) {
      setDashboard(safeDashboard)
    }
  }, [allowMemberDetail, dashboard, storageKey])

  const totals = visibleData?.totals
  const completed = (totals?.successes ?? 0) + (totals?.errors ?? 0)
  const successRate = completed
    ? Math.round(((totals?.successes ?? 0) / completed) * 100)
    : 0
  const pricingCoverage = totals?.model_calls
    ? Math.max(
        0,
        Math.round(
          ((totals.model_calls - totals.unpriced_model_calls) / totals.model_calls) * 100,
        ),
      )
    : 0
  const empty = !loading && !hasAnalyticsData(visibleData)
  const visibleWidgets = dashboard.widgets.filter(
    (widget) => allowMemberDetail || widget.id !== "members",
  )
  const hiddenWidgets = (Object.keys(WIDGET_META) as WidgetId[]).filter(
    (id) =>
      (allowMemberDetail || id !== "members") &&
      !visibleWidgets.some((widget) => widget.id === id),
  )

  function updateWidgets(update: (widgets: WidgetState[]) => WidgetState[]) {
    setDashboard((current) => ({
      ...current,
      preset: "custom",
      widgets: update(current.widgets),
    }))
  }

  function applyPreset(preset: DashboardPreset) {
    if (preset === "custom") return
    setSavedViewId(null)
    setDashboard((current) => ({
      preset,
      density: current.density,
      widgets: dashboardForAccess(
        {
          preset,
          density: current.density,
          widgets: PRESET_WIDGETS[preset].map((widget) => ({ ...widget })),
        },
        allowMemberDetail,
      ).widgets,
    }))
  }

  function applySelection(value: string) {
    if (!value.startsWith("saved:")) {
      applyPreset(value as DashboardPreset)
      return
    }
    const view = visibleSavedViews.find((item) => item.id === value.slice(6))
    if (!view) return
    setSavedViewId(view.id)
    setDashboard(
      dashboardForAccess(dashboardFromDefinition(view.definition), allowMemberDetail),
    )
    onApplyQuery?.(sanitizeAnalyticsQuery(view.definition.query, allowMemberDetail)!)
  }

  return (
    <section className="mt-4" aria-labelledby="analytics-studio-title">
      <div className="rounded-xl border border-(--border-card) bg-(--bg-card) p-3 sm:p-4">
        <div className="flex flex-col gap-3 xl:flex-row xl:items-center xl:justify-between">
          <div className="min-w-0">
            <div className="mb-1 flex items-center gap-2">
              <span className="grid size-7 place-items-center rounded-lg bg-(--color-accent-soft) text-(--color-accent)">
                <LayoutDashboard className="size-4" />
              </span>
              <h2 id="analytics-studio-title" className="text-base font-semibold tracking-tight">
                Analytics Studio
              </h2>
              <Badge tone={savedViewId ? "success" : "accent"}>
                {savedViewId ? "Saved to Conductor" : "Browser draft"}
              </Badge>
            </div>
            <p className="max-w-2xl text-xs leading-relaxed text-(--color-text-muted)">
              Compose a decision-ready dashboard from privacy-safe EvoFlux telemetry. Layout,
              chart types and density can be saved privately or shared with the project.
            </p>
          </div>

          <div className="flex flex-wrap items-center gap-2">
            <Select
              value={savedViewId ? `saved:${savedViewId}` : dashboard.preset}
              onValueChange={applySelection}
              options={[
                ...PRESET_OPTIONS,
                ...visibleSavedViews.map((view) => ({
                  value: `saved:${view.id}`,
                  label: `${view.visibility === "shared" ? "Shared" : "Mine"} · ${view.name}`,
                })),
              ]}
              className="w-56"
              aria-label="Dashboard preset"
            />
            <Button
              variant={customizing ? "secondary" : "outline"}
              onClick={() => setCustomizing((value) => !value)}
              aria-pressed={customizing}
            >
              <Settings2 className="size-3.5" />
              Customize
            </Button>
            {canManageSelectedView && (
              <Button
                variant={savedViewId ? "outline" : "default"}
                disabled={createView.isPending || updateView.isPending}
                onClick={() => {
                  if (savedViewId) updateView.mutate()
                  else {
                    setViewName(`${scopeLabel} dashboard`)
                    setShowSaveView(true)
                  }
                }}
              >
                <Save className="size-3.5" />
                {updateView.isPending ? "Saving…" : savedViewId ? "Save changes" : "Save view"}
              </Button>
            )}
            <Menu
              side="bottom"
              align="end"
              trigger={
                <Button variant="outline" disabled={!visibleData}>
                  <Download className="size-3.5" /> Export
                </Button>
              }
            >
              <MenuGroup>
                <MenuGroupLabel>Portable report data</MenuGroupLabel>
                <MenuItem onClick={() => visibleData && exportResourceAnalytics(visibleData, "csv", scopeLabel)}>
                  <FileSpreadsheet className="size-4" /> Export CSV
                </MenuItem>
                <MenuItem onClick={() => visibleData && exportResourceAnalytics(visibleData, "json", scopeLabel)}>
                  <FileJson className="size-4" /> Export JSON
                </MenuItem>
                <MenuSeparator />
                <MenuItem onClick={() => window.print()}>
                  <Download className="size-4" /> Print / save PDF
                </MenuItem>
              </MenuGroup>
            </Menu>
          </div>
        </div>

        {(savedViews.error || createView.error || updateView.error || deleteView.error) && (
          <div className="mt-3 rounded-lg border border-(--color-error)/30 bg-(--color-error-subtle) px-3 py-2 text-xs text-(--color-error)">
            {[savedViews.error, createView.error, updateView.error, deleteView.error]
              .find((error) => error instanceof Error)?.message ?? "Saved view action failed"}
          </div>
        )}

        {customizing && (
          <div className="mt-4 flex flex-col gap-3 rounded-lg border border-(--color-accent)/25 bg-(--color-accent-soft)/25 p-3 lg:flex-row lg:items-center lg:justify-between">
            <div>
              <div className="text-xs font-semibold">Dashboard layout</div>
              <p className="mt-0.5 text-[0.7rem] text-(--color-text-muted)">
                Reorder, resize, change visualization or remove any widget. Changes save automatically.
              </p>
            </div>
            <div className="flex flex-wrap gap-2">
              <Select
                value={dashboard.density}
                onValueChange={(density) =>
                  setDashboard((current) => ({ ...current, density }))
                }
                options={[
                  { value: "comfortable", label: "Comfortable" },
                  { value: "compact", label: "Compact" },
                ]}
                className="w-36"
                aria-label="Dashboard density"
              />
              <Menu
                side="bottom"
                align="end"
                trigger={
                  <Button variant="outline" disabled={hiddenWidgets.length === 0}>
                    <Plus className="size-3.5" /> Add chart
                  </Button>
                }
              >
                <MenuGroup>
                  <MenuGroupLabel>Widget library</MenuGroupLabel>
                  {hiddenWidgets.map((id) => (
                    <MenuItem
                      key={id}
                      onClick={() =>
                        updateWidgets((widgets) => [
                          ...widgets,
                          {
                            id,
                            width: "half",
                            view: WIDGET_META[id].defaultView,
                          },
                        ])
                      }
                    >
                      <BarChart3 className="size-4" /> {WIDGET_META[id].title}
                    </MenuItem>
                  ))}
                </MenuGroup>
              </Menu>
              <Button
                variant="ghost"
                onClick={() =>
                  setDashboard(
                    dashboardForAccess(defaultDashboard("executive"), allowMemberDetail),
                  )
                }
              >
                <RotateCcw className="size-3.5" /> Reset
              </Button>
              {selectedSavedView && canManageSelectedView && (
                <Button
                  variant="destructive"
                  disabled={deleteView.isPending}
                  onClick={() => {
                    if (window.confirm(`Delete saved view “${selectedSavedView.name}”?`)) {
                      deleteView.mutate()
                    }
                  }}
                >
                  <X className="size-3.5" /> Delete view
                </Button>
              )}
            </div>
          </div>
        )}
      </div>

      <div className="mt-3 grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
        <InsightMetric
          label="Attributed requests"
          value={(totals?.requests ?? 0).toLocaleString()}
          hint={`${(totals?.resource_uses ?? 0).toLocaleString()} resource uses`}
          icon={Gauge}
        />
        <InsightMetric
          label="Active adoption"
          value={(totals?.installed_members ?? 0).toLocaleString()}
          hint={`${(totals?.installed_installations ?? 0).toLocaleString()} installations in sync`}
          icon={CheckCircle2}
          tone="success"
        />
        <InsightMetric
          label="Reliability"
          value={completed ? `${successRate}%` : "—"}
          hint={`${totals?.errors ?? 0} errors · ${totals?.blocked ?? 0} blocked`}
          icon={Sparkles}
          tone={completed && successRate < 90 ? "warning" : "success"}
        />
        <InsightMetric
          label="Cost coverage"
          value={totals?.model_calls ? `${pricingCoverage}%` : "—"}
          hint={`${formatEstimatedCost(totals?.estimated_cost_usd_micros ?? 0)} estimated · ${totals?.unpriced_model_calls ?? 0} unpriced`}
          icon={FileSpreadsheet}
          tone="accent"
        />
      </div>

      {loading ? (
        <div className="mt-3 grid gap-3 lg:grid-cols-2">
          {[0, 1, 2, 3].map((item) => (
            <div key={item} className="h-64 animate-pulse rounded-xl border border-(--border-card) bg-(--bg-card)" />
          ))}
        </div>
      ) : empty ? (
        <TelemetryReadiness data={visibleData} />
      ) : (
        <div className="mt-3 grid grid-cols-1 gap-3 lg:grid-cols-12">
          {visibleWidgets.map((widget, index) => (
            <div
              key={widget.id}
              className={widget.width === "full" ? "lg:col-span-12" : "lg:col-span-6"}
            >
              <AnalyticsWidget
                widget={widget}
                index={index}
                count={visibleWidgets.length}
                data={visibleData!}
                density={dashboard.density}
                customizing={customizing}
                onChange={(next) =>
                  updateWidgets((widgets) =>
                    widgets.map((item) => (item.id === widget.id ? next : item)),
                  )
                }
                onMove={(direction) =>
                  updateWidgets((widgets) => moveWidget(widgets, index, direction))
                }
                onRemove={() =>
                  updateWidgets((widgets) => widgets.filter((item) => item.id !== widget.id))
                }
              />
            </div>
          ))}
        </div>
      )}

      <Dialog
        open={showSaveView && canCreateView}
        title="Save analytics view"
        description="Persist this dashboard on Conductor so it follows you across browsers and can be shared with the project."
        onClose={() => {
          setShowSaveView(false)
          createView.reset()
        }}
        footer={
          <>
            <Button
              variant="ghost"
              disabled={createView.isPending}
              onClick={() => setShowSaveView(false)}
            >
              Cancel
            </Button>
            <Button
              disabled={!viewName.trim() || createView.isPending}
              onClick={() => createView.mutate()}
            >
              <Save className="size-3.5" />
              {createView.isPending ? "Saving…" : "Save view"}
            </Button>
          </>
        }
      >
        <div className="space-y-4">
          <div className="space-y-1.5">
            <Label htmlFor="analytics-view-name">View name</Label>
            <Input
              id="analytics-view-name"
              value={viewName}
              maxLength={120}
              autoFocus
              placeholder="Quarterly adoption dashboard"
              onChange={(event) => setViewName(event.target.value)}
            />
          </div>
          <div className="space-y-1.5">
            <Label htmlFor="analytics-view-visibility">Visibility</Label>
            <Select
              id="analytics-view-visibility"
              value={viewVisibility}
              onValueChange={setViewVisibility}
              options={[
                { value: "private", label: "Private · only me" },
                { value: "shared", label: "Shared · everyone in this project" },
              ]}
            />
          </div>
          <p className="text-xs leading-relaxed text-(--color-text-muted)">
            The saved definition contains filters, metrics and layout only. Telemetry rows and
            private request content are never copied into the view.
          </p>
          {createView.error && (
            <div className="rounded-lg border border-(--color-error)/30 bg-(--color-error-subtle) px-3 py-2 text-xs text-(--color-error)">
              {createView.error instanceof Error ? createView.error.message : "Unable to save view"}
            </div>
          )}
        </div>
      </Dialog>
    </section>
  )
}

function AnalyticsWidget({
  widget,
  index,
  count,
  data,
  density,
  customizing,
  onChange,
  onMove,
  onRemove,
}: {
  widget: WidgetState
  index: number
  count: number
  data: ResourceUsageAnalytics
  density: DashboardDensity
  customizing: boolean
  onChange: (widget: WidgetState) => void
  onMove: (direction: -1 | 1) => void
  onRemove: () => void
}) {
  const meta = WIDGET_META[widget.id]
  return (
    <Card className={cn("h-full overflow-hidden", customizing && "ring-1 ring-(--color-accent)/20")}>
      <CardHeader className="min-h-16">
        <div className="min-w-0">
          <CardTitle>{meta.title}</CardTitle>
          <CardDescription className="mt-0.5">{meta.description}</CardDescription>
        </div>
        {customizing && (
          <div className="flex flex-wrap items-center justify-end gap-1">
            <Select
              value={widget.view}
              onValueChange={(view) => onChange({ ...widget, view })}
              options={meta.views.map((view) => ({
                value: view,
                label: view.charAt(0).toUpperCase() + view.slice(1),
              }))}
              className="w-24"
              aria-label={`Visualization for ${meta.title}`}
            />
            <Button
              variant="ghost"
              size="icon"
              title="Move earlier"
              aria-label={`Move ${meta.title} earlier`}
              disabled={index === 0}
              onClick={() => onMove(-1)}
            >
              <ArrowUp className="size-3.5" />
            </Button>
            <Button
              variant="ghost"
              size="icon"
              title="Move later"
              aria-label={`Move ${meta.title} later`}
              disabled={index === count - 1}
              onClick={() => onMove(1)}
            >
              <ArrowDown className="size-3.5" />
            </Button>
            <Button
              variant="ghost"
              size="icon"
              title={widget.width === "full" ? "Use half width" : "Use full width"}
              aria-label={widget.width === "full" ? `Shrink ${meta.title}` : `Expand ${meta.title}`}
              onClick={() =>
                onChange({ ...widget, width: widget.width === "full" ? "half" : "full" })
              }
            >
              {widget.width === "full" ? <Columns2 className="size-3.5" /> : <Maximize2 className="size-3.5" />}
            </Button>
            <Button
              variant="ghost"
              size="icon"
              title="Remove widget"
              aria-label={`Remove ${meta.title}`}
              onClick={onRemove}
            >
              <X className="size-3.5" />
            </Button>
          </div>
        )}
      </CardHeader>
      <CardContent className={density === "compact" ? "p-3" : "p-4"}>
        <WidgetVisualization widget={widget} data={data} density={density} />
      </CardContent>
    </Card>
  )
}

function WidgetVisualization({
  widget,
  data,
  density,
}: {
  widget: WidgetState
  data: ResourceUsageAnalytics
  density: DashboardDensity
}) {
  if (widget.id === "requests" || widget.id === "outcomes" || widget.id === "consumption") {
    return <DailyWidget id={widget.id} view={widget.view} daily={data.daily} density={density} />
  }
  return <RankingWidget id={widget.id} view={widget.view} data={data} density={density} />
}

function DailyWidget({
  id,
  view,
  daily,
  density,
}: {
  id: "requests" | "outcomes" | "consumption"
  view: WidgetView
  daily: ResourceUsageDay[]
  density: DashboardDensity
}) {
  const height = density === "compact" ? "h-48" : "h-60"
  const consumptionRows = daily.map((item) => ({
    ...item,
    total_tokens:
      item.tokens_in +
      item.tokens_out +
      item.cache_read_tokens +
      item.reasoning_tokens +
      item.tool_use_tokens,
  }))
  const rows = id === "consumption" ? consumptionRows : daily
  const config: ChartConfig = id === "requests"
    ? REQUEST_CONFIG
    : id === "outcomes"
      ? OUTCOME_CONFIG
      : CONSUMPTION_CONFIG
  const keys = Object.keys(config)

  if (view === "table") {
    return (
      <VisibleDataTable
        columns={["date", ...keys]}
        rows={rows.map((item) => ({ ...item }))}
        maxRows={density === "compact" ? 6 : 9}
      />
    )
  }

  if (id === "consumption") {
    return (
      <ConsumptionChart rows={consumptionRows} view={view} config={config} className={height} />
    )
  }

  const common = {
    data: rows,
    margin: { top: 8, right: 8, bottom: 0, left: 0 },
  }
  const axes = (
    <>
      <CartesianGrid vertical={false} stroke="var(--border-soft)" strokeDasharray="3 3" />
      <XAxis dataKey="date" axisLine={false} tickLine={false} tickFormatter={formatShortDate} tick={{ fill: "var(--color-text-subtle)", fontSize: 10 }} />
      <YAxis allowDecimals={false} axisLine={false} tickLine={false} width={36} tick={{ fill: "var(--color-text-subtle)", fontSize: 10 }} />
      <ChartTooltip content={<ChartTooltipContent config={config} />} />
    </>
  )

  return (
    <>
      <ChartContainer config={config} className={cn(height, "w-full")}>
        {view === "bar" ? (
          <BarChart accessibilityLayer {...common}>
            {axes}
            {keys.map((key, index) => (
              <Bar key={key} dataKey={key} stackId={id === "outcomes" ? "outcome" : undefined} fill={config[key].color} radius={index === keys.length - 1 ? [3, 3, 0, 0] : 0} />
            ))}
          </BarChart>
        ) : view === "line" ? (
          <LineChart accessibilityLayer {...common}>
            {axes}
            {keys.map((key) => (
              <Line key={key} type="monotone" dataKey={key} stroke={config[key].color} strokeWidth={2} dot={false} />
            ))}
          </LineChart>
        ) : (
          <AreaChart accessibilityLayer {...common}>
            {axes}
            {keys.map((key) => (
              <Area key={key} type="monotone" dataKey={key} stackId={id === "outcomes" ? "outcome" : undefined} fill={config[key].color} fillOpacity={0.18} stroke={config[key].color} />
            ))}
          </AreaChart>
        )}
      </ChartContainer>
      <SeriesLegend config={config} />
      <AccessibleChartTable
        caption={`${WIDGET_META[id].title} data`}
        rows={rows.map((item) => ({ ...item }))}
        columns={[{ key: "date", label: "Date" }, ...keys.map((key) => ({ key, label: String(config[key].label) }))]}
      />
    </>
  )
}

function ConsumptionChart({
  rows,
  view,
  config,
  className,
}: {
  rows: Array<ResourceUsageDay & { total_tokens: number }>
  view: WidgetView
  config: ChartConfig
  className: string
}) {
  const axes = (
    <>
      <CartesianGrid vertical={false} stroke="var(--border-soft)" strokeDasharray="3 3" />
      <XAxis dataKey="date" axisLine={false} tickLine={false} tickFormatter={formatShortDate} tick={{ fill: "var(--color-text-subtle)", fontSize: 10 }} />
      <YAxis yAxisId="tokens" axisLine={false} tickLine={false} width={46} tickFormatter={formatTokens} tick={{ fill: "var(--color-text-subtle)", fontSize: 10 }} />
      <YAxis yAxisId="cost" orientation="right" axisLine={false} tickLine={false} width={58} tickFormatter={formatEstimatedCost} tick={{ fill: "var(--color-text-subtle)", fontSize: 10 }} />
      <ChartTooltip content={<ChartTooltipContent config={config} valueFormatter={(value) => typeof value === "number" ? value.toLocaleString() : String(value)} />} />
    </>
  )
  const common = { data: rows, margin: { top: 8, right: 8, bottom: 0, left: 0 } }
  return (
    <>
      <ChartContainer config={config} className={cn(className, "w-full")}>
        {view === "bar" ? (
          <BarChart accessibilityLayer {...common}>
            {axes}
            <Bar yAxisId="tokens" dataKey="total_tokens" fill="var(--chart-series-2)" radius={[3, 3, 0, 0]} />
            <Line yAxisId="cost" type="monotone" dataKey="estimated_cost_usd_micros" stroke="var(--chart-series-5)" strokeWidth={2} dot={false} />
          </BarChart>
        ) : view === "line" ? (
          <LineChart accessibilityLayer {...common}>
            {axes}
            <Line yAxisId="tokens" type="monotone" dataKey="total_tokens" stroke="var(--chart-series-2)" strokeWidth={2} dot={false} />
            <Line yAxisId="cost" type="monotone" dataKey="estimated_cost_usd_micros" stroke="var(--chart-series-5)" strokeWidth={2} dot={false} />
          </LineChart>
        ) : (
          <AreaChart accessibilityLayer {...common}>
            {axes}
            <Area yAxisId="tokens" type="monotone" dataKey="total_tokens" fill="var(--chart-series-2)" fillOpacity={0.2} stroke="var(--chart-series-2)" />
            <Line yAxisId="cost" type="monotone" dataKey="estimated_cost_usd_micros" stroke="var(--chart-series-5)" strokeWidth={2} dot={false} />
          </AreaChart>
        )}
      </ChartContainer>
      <SeriesLegend config={config} />
      <AccessibleChartTable caption="Daily token and cost data" rows={rows.map((item) => ({ ...item }))} columns={[{ key: "date", label: "Date" }, { key: "total_tokens", label: "Tokens" }, { key: "estimated_cost_usd_micros", label: "Estimated cost in USD micros" }]} />
    </>
  )
}

function RankingWidget({
  id,
  view,
  data,
  density,
}: {
  id: Exclude<WidgetId, "requests" | "outcomes" | "consumption">
  view: WidgetView
  data: ResourceUsageAnalytics
  density: DashboardDensity
}) {
  const rows = rankingRows(id, data).slice(0, density === "compact" ? 6 : 8)
  const config = { value: { label: rankingMeasureLabel(id), color: "var(--chart-series-1)" } } satisfies ChartConfig
  const height = density === "compact" ? "h-48" : "h-60"

  if (view === "table") {
    return <VisibleDataTable columns={["label", "detail", "value"]} rows={rows} maxRows={density === "compact" ? 6 : 8} />
  }
  if (view === "donut") {
    const colored = rows.map((row, index) => ({
      ...row,
      color: TELEMETRY_CHART_SERIES[index % TELEMETRY_CHART_SERIES.length],
    }))
    const total = colored.reduce((sum, item) => sum + item.value, 0)
    return (
      <div className="grid gap-3 sm:grid-cols-[12rem_minmax(0,1fr)] sm:items-center">
        <div className="relative">
          <ChartContainer config={config} className="h-48 w-full">
            <PieChart accessibilityLayer>
              <ChartTooltip content={<ChartTooltipContent config={config} />} />
              <Pie data={colored} dataKey="value" nameKey="label" innerRadius={46} outerRadius={72} paddingAngle={2} stroke="var(--bg-card)" strokeWidth={2}>
                {colored.map((item) => <Cell key={item.key} fill={item.color} />)}
              </Pie>
            </PieChart>
          </ChartContainer>
          <div className="pointer-events-none absolute inset-0 grid place-items-center text-center">
            <div><div className="text-xl font-semibold tabular-nums">{total.toLocaleString()}</div><div className="text-[0.65rem] text-(--color-text-subtle)">{rankingMeasureLabel(id)}</div></div>
          </div>
        </div>
        <ChartLegendList items={colored.map((item) => ({ key: item.key, label: item.label, color: item.color, value: item.value.toLocaleString() }))} />
        <AccessibleChartTable caption={`${WIDGET_META[id].title} data`} rows={rows} columns={[{ key: "label", label: "Name" }, { key: "detail", label: "Detail" }, { key: "value", label: rankingMeasureLabel(id) }]} />
      </div>
    )
  }

  return (
    <>
      <ChartContainer config={config} className={cn(height, "w-full")}>
        <BarChart accessibilityLayer data={rows} layout="vertical" margin={{ top: 4, right: 8, bottom: 0, left: 12 }}>
          <CartesianGrid horizontal={false} stroke="var(--border-soft)" strokeDasharray="3 3" />
          <XAxis type="number" allowDecimals={false} axisLine={false} tickLine={false} tick={{ fill: "var(--color-text-subtle)", fontSize: 10 }} />
          <YAxis type="category" dataKey="label" width={96} axisLine={false} tickLine={false} tick={{ fill: "var(--color-text-subtle)", fontSize: 10 }} />
          <ChartTooltip content={<ChartTooltipContent config={config} />} />
          <Bar dataKey="value" fill="var(--chart-series-1)" radius={[0, 4, 4, 0]} />
        </BarChart>
      </ChartContainer>
      <AccessibleChartTable caption={`${WIDGET_META[id].title} data`} rows={rows} columns={[{ key: "label", label: "Name" }, { key: "detail", label: "Detail" }, { key: "value", label: rankingMeasureLabel(id) }]} />
    </>
  )
}

function VisibleDataTable({
  columns,
  rows,
  maxRows,
}: {
  columns: string[]
  rows: Array<Record<string, unknown>>
  maxRows: number
}) {
  return (
    <div className="max-h-64 overflow-auto rounded-lg border border-(--border-soft)">
      <table className="w-full border-collapse text-left text-xs">
        <thead className="sticky top-0 bg-(--bg-key) text-[0.68rem] text-(--color-text-muted)">
          <tr>{columns.map((column) => <th key={column} className="border-b border-(--border-soft) px-3 py-2 font-medium capitalize">{column.replaceAll("_", " ")}</th>)}</tr>
        </thead>
        <tbody className="divide-y divide-(--border-soft)">
          {rows.slice(0, maxRows).map((row, index) => (
            <tr key={String(row.key ?? row.date ?? index)}>
              {columns.map((column) => (
                <td key={column} className="max-w-56 truncate px-3 py-2 tabular-nums text-(--color-text-muted)">
                  {formatCell(row[column])}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  )
}

export function TelemetryReadiness({ data }: { data?: ResourceUsageAnalytics }) {
  const reported = data?.totals.reported_installations ?? 0
  const installed = data?.totals.installed_installations ?? 0
  return (
    <Card className="mt-3 overflow-hidden">
      <div className="grid lg:grid-cols-[minmax(0,1.1fr)_minmax(24rem,0.9fr)]">
        <div className="p-5 sm:p-6">
          <div className="mb-3 flex items-center gap-2">
            <span className="grid size-9 place-items-center rounded-lg bg-(--color-accent-soft) text-(--color-accent)">
              <Sparkles className="size-4" />
            </span>
            <div>
              <h3 className="text-sm font-semibold">Dashboard ready for its first signal</h3>
              <p className="mt-0.5 text-xs text-(--color-text-muted)">No attributed usage arrived in the selected range.</p>
            </div>
          </div>
          <p className="max-w-2xl text-xs leading-relaxed text-(--color-text-muted)">
            Conductor only visualizes server-attributed operational metadata. Connect an EvoFlux
            installation, pull a governed version, then report request outcomes to populate every widget.
          </p>
          <div className="mt-4 flex flex-wrap gap-2 text-[0.7rem]">
            <Badge tone={reported > 0 ? "success" : "neutral"}>{reported} reporting installations</Badge>
            <Badge tone={installed > 0 ? "success" : "warning"}>{installed} installed</Badge>
            <Badge tone="neutral">0 attributed requests</Badge>
          </div>
        </div>
        <ol className="border-t border-(--border-soft) bg-(--bg-key)/45 p-4 lg:border-t-0 lg:border-l">
          <ReadinessStep number="1" title="Connect EvoFlux" description="Issue a scoped connection secret and establish the delivery stream." done={reported > 0} />
          <ReadinessStep number="2" title="Reconcile a release" description="Confirm the desired version is applied to at least one installation." done={installed > 0} />
          <ReadinessStep number="3" title="Report an outcome" description="Send idempotent, privacy-safe request metadata after execution." done={false} last />
        </ol>
      </div>
    </Card>
  )
}

function ReadinessStep({
  number,
  title,
  description,
  done,
  last = false,
}: {
  number: string
  title: string
  description: string
  done: boolean
  last?: boolean
}) {
  return (
    <li className={cn("relative flex gap-3 pb-4", !last && "after:absolute after:top-7 after:bottom-0 after:left-3 after:w-px after:bg-(--border-soft)")}>
      <span className={cn("relative z-10 grid size-6 shrink-0 place-items-center rounded-full border text-[0.65rem] font-semibold", done ? "border-(--color-success)/40 bg-(--color-success)/12 text-(--color-success)" : "border-(--color-border) bg-(--bg-card) text-(--color-text-subtle)")}>
        {done ? <CheckCircle2 className="size-3.5" /> : number}
      </span>
      <div>
        <div className="text-xs font-medium">{title}</div>
        <p className="mt-0.5 text-[0.7rem] leading-relaxed text-(--color-text-muted)">{description}</p>
      </div>
    </li>
  )
}

function InsightMetric({
  label,
  value,
  hint,
  icon: Icon,
  tone = "neutral",
}: {
  label: string
  value: string
  hint: string
  icon: typeof Gauge
  tone?: "neutral" | "accent" | "success" | "warning"
}) {
  const tones = {
    neutral: "bg-(--bg-key) text-(--color-text-subtle)",
    accent: "bg-(--color-accent-soft) text-(--color-accent)",
    success: "bg-(--color-success)/12 text-(--color-success)",
    warning: "bg-(--color-warning)/12 text-(--color-warning)",
  }
  return (
    <div className="rounded-xl border border-(--border-card) bg-(--bg-card) p-4">
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0">
          <div className="text-xs font-medium text-(--color-text-muted)">{label}</div>
          <div className="mt-2 text-2xl font-semibold tracking-tight tabular-nums">{value}</div>
          <p className="mt-1 text-[0.7rem] leading-relaxed text-(--color-text-subtle)">{hint}</p>
        </div>
        <span className={cn("grid size-8 shrink-0 place-items-center rounded-lg", tones[tone])}>
          <Icon className="size-4" />
        </span>
      </div>
    </div>
  )
}

function SeriesLegend({ config }: { config: ChartConfig }) {
  return (
    <ChartLegendList
      className="mt-3 flex flex-wrap justify-center gap-x-5"
      items={Object.entries(config).map(([key, item]) => ({
        key,
        label: item.label,
        color: item.color,
      }))}
    />
  )
}

function rankingRows(
  id: Exclude<WidgetId, "requests" | "outcomes" | "consumption">,
  data: ResourceUsageAnalytics,
) {
  if (id === "resources") {
    return data.resources.map((item) => ({ key: `${item.resource_id}:${item.version_id}:${item.relation}`, label: item.name, detail: `v${item.version} · ${item.kind}`, value: item.uses }))
  }
  if (id === "members") {
    return data.members.map((item) => ({ key: item.user_id, label: item.display_name, detail: item.primary_role, value: item.requests }))
  }
  if (id === "models") {
    return data.models.map((item) => ({ key: `${item.provider}:${item.model}`, label: item.model, detail: item.provider, value: item.calls }))
  }
  if (id === "tools") {
    return data.tools.map((item) => ({ key: item.tool_name, label: item.tool_name, detail: `${item.category} · ${item.errors} errors`, value: item.calls }))
  }
  return data.roles.map((item) => ({ key: item.primary_role, label: item.primary_role, detail: `${item.model_calls} model · ${item.tool_calls} tool`, value: item.model_calls + item.tool_calls }))
}

function rankingMeasureLabel(id: Exclude<WidgetId, "requests" | "outcomes" | "consumption">) {
  if (id === "resources") return "uses"
  if (id === "members") return "requests"
  return "calls"
}

function formatCell(value: unknown) {
  if (typeof value === "number") return value.toLocaleString()
  if (value == null || value === "") return "—"
  return String(value)
}

export function hasAnalyticsData(data?: ResourceUsageAnalytics) {
  if (!data) return false
  return Boolean(
    data.totals.requests ||
    data.totals.resource_uses ||
    data.totals.model_calls ||
    data.totals.tool_calls,
  )
}

function moveWidget(widgets: WidgetState[], index: number, direction: -1 | 1) {
  const nextIndex = index + direction
  if (nextIndex < 0 || nextIndex >= widgets.length) return widgets
  const next = [...widgets]
  const current = next[index]
  next[index] = next[nextIndex]
  next[nextIndex] = current
  return next
}

function defaultDashboard(preset: Exclude<DashboardPreset, "custom">): DashboardState {
  return {
    preset,
    density: "comfortable",
    widgets: PRESET_WIDGETS[preset].map((widget) => ({ ...widget })),
  }
}

function dashboardForAccess(
  dashboard: DashboardState,
  allowMemberDetail: boolean,
): DashboardState {
  if (allowMemberDetail) return dashboard
  return {
    ...dashboard,
    widgets: dashboard.widgets.filter((widget) => widget.id !== "members"),
  }
}

function sanitizeAnalyticsQuery(
  query: AnalyticsViewDefinition["query"] | undefined,
  allowMemberDetail: boolean,
): AnalyticsViewDefinition["query"] | undefined {
  if (!query || allowMemberDetail) return query
  return {
    ...query,
    member_id: null,
    installation_id: null,
  }
}

function analyticsDataForAccess(
  data: ResourceUsageAnalytics | undefined,
  allowMemberDetail: boolean,
): ResourceUsageAnalytics | undefined {
  if (!data || allowMemberDetail) return data
  return {
    ...data,
    members: [],
    activity: [],
    activity_total: 0,
  }
}

const WIDGET_CONTRACT: Record<
  WidgetId,
  {
    metric: AnalyticsViewDefinition["widgets"][number]["metric"]
    groupBy: AnalyticsViewDefinition["widgets"][number]["group_by"]
  }
> = {
  requests: { metric: "requests", groupBy: "time" },
  outcomes: { metric: "requests", groupBy: "outcome" },
  consumption: { metric: "total_tokens", groupBy: "time" },
  resources: { metric: "resource_uses", groupBy: "resource" },
  members: { metric: "requests", groupBy: "member" },
  models: { metric: "model_calls", groupBy: "model" },
  tools: { metric: "tool_calls", groupBy: "tool" },
  roles: { metric: "model_calls", groupBy: "role" },
}

function dashboardDefinition(
  dashboard: DashboardState,
  query?: AnalyticsViewDefinition["query"],
  scope?: { resourceKind?: ResourceKind; resourceId?: string },
): AnalyticsViewDefinition {
  return {
    schema_version: 1,
    preset: dashboard.preset,
    density: dashboard.density,
    query: query ?? {
      date_range: "last_30_days",
      resource_kind: scope?.resourceKind ?? null,
      resource_id: scope?.resourceId ?? null,
    },
    widgets: dashboard.widgets.map((widget) => {
      const contract = WIDGET_CONTRACT[widget.id]
      return {
        id: widget.id,
        title: WIDGET_META[widget.id].title,
        visualization: widget.view === "bar" && widget.id === "outcomes"
          ? "stacked_bar"
          : widget.view,
        metric: contract.metric,
        group_by: contract.groupBy,
        size: widget.width === "full" ? "full" : "half",
        limit: 10,
        show_legend: true,
      }
    }),
  }
}

function savedViewMatchesScope(
  view: AnalyticsView,
  scope?: { resourceKind?: ResourceKind; resourceId?: string },
) {
  if (!scope) return true
  const query = view.definition.query
  if (scope.resourceId) return query.resource_id === scope.resourceId
  return query.resource_kind === scope.resourceKind && !query.resource_id
}

function dashboardFromDefinition(definition: AnalyticsViewDefinition): DashboardState {
  const widgets = definition.widgets.flatMap<WidgetState>((widget) => {
    if (!(widget.id in WIDGET_META)) return []
    const id = widget.id as WidgetId
    const requestedView: WidgetView = widget.visualization === "stacked_bar"
      ? "bar"
      : widget.visualization === "kpi"
        ? WIDGET_META[id].defaultView
        : widget.visualization
    const view = WIDGET_META[id].views.includes(requestedView)
      ? requestedView
      : WIDGET_META[id].defaultView
    return [{
      id,
      view,
      width: widget.size === "full" || widget.size === "two_thirds" ? "full" : "half",
    }]
  })

  return widgets.length
    ? {
        preset: definition.preset,
        density: definition.density,
        widgets,
      }
    : defaultDashboard("executive")
}

function readDashboard(storageKey: string): DashboardState | null {
  try {
    const value = JSON.parse(window.localStorage.getItem(storageKey) ?? "null") as Partial<DashboardState> | null
    if (!value || !Array.isArray(value.widgets)) return null
    const widgets = value.widgets.filter(isWidgetState)
    if (!widgets.length) return null
    const preset = PRESET_OPTIONS.some((option) => option.value === value.preset)
      ? value.preset as DashboardPreset
      : "custom"
    return {
      preset,
      density: value.density === "compact" ? "compact" : "comfortable",
      widgets,
    }
  } catch {
    return null
  }
}

function isWidgetState(value: unknown): value is WidgetState {
  if (!value || typeof value !== "object") return false
  const widget = value as Partial<WidgetState>
  return Boolean(
    widget.id &&
    widget.id in WIDGET_META &&
    (widget.width === "half" || widget.width === "full") &&
    widget.view &&
    WIDGET_META[widget.id as WidgetId].views.includes(widget.view),
  )
}
