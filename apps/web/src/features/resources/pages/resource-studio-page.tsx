import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"
import { useNavigate, useParams } from "@tanstack/react-router"
import {
  AlertTriangle,
  ArrowLeft,
  CheckCircle2,
  ChevronRight,
  CircleDot,
  FileCode2,
  GitBranch,
  PackageCheck,
  Rocket,
  Save,
  ShieldCheck,
  Sparkles,
  Upload,
  Users,
} from "lucide-react"
import { lazy, Suspense, useEffect, useRef, useState } from "react"

import {
  api,
  type DraftFileTree,
  type ManagedResource,
  type ReleaseResourceRequest,
  type ResourceValidation,
} from "@/shared/api/client"
import {
  RELEASE_CHANNEL,
  RESOURCE_IMPORT_ACCEPT,
  RESOURCE_KIND,
  RESOURCE_KIND_LABEL,
  RESOURCE_QUERY_KEY,
  VERSION_MODE,
  type ReleaseChannel,
  type VersionMode,
} from "@/shared/constants/resource"
import { PageFrame } from "@/shared/components/page-frame"
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

const MonacoEditor = lazy(() => import("@/shared/components/code-editor"))

type StudioTab = "studio" | "releases"

export function ResourceStudioPage() {
  const { resourceId } = useParams({ strict: false }) as {
    resourceId: string
  }
  const navigate = useNavigate()
  const queryClient = useQueryClient()
  const [tab, setTab] = useState<StudioTab>("studio")
  const [selectedPath, setSelectedPath] = useState<string | null>(null)
  const [editorValue, setEditorValue] = useState("")
  const [dirty, setDirty] = useState(false)
  const [validation, setValidation] = useState<ResourceValidation | null>(null)
  const [showRelease, setShowRelease] = useState(false)
  const [releaseResult, setReleaseResult] = useState<string | null>(null)
  const importInput = useRef<HTMLInputElement>(null)

  const resources = useQuery({
    queryKey: [RESOURCE_QUERY_KEY],
    queryFn: () => api.resources(),
  })
  const resource = resources.data?.find((item) => item.id === resourceId)
  const draft = useQuery({
    queryKey: [RESOURCE_QUERY_KEY, resourceId, "draft"],
    queryFn: () => api.resourceDraft(resourceId),
    enabled: Boolean(resource),
  })
  const versions = useQuery({
    queryKey: [RESOURCE_QUERY_KEY, resourceId, "versions"],
    queryFn: () => api.resourceVersions(resourceId),
    enabled: Boolean(resource),
  })

  useEffect(() => {
    if (!draft.data?.files.length) return
    const selected =
      draft.data.files.find((file) => file.path === selectedPath) ?? draft.data.files[0]
    setSelectedPath(selected.path)
    setEditorValue(selected.content)
    setDirty(false)
  }, [draft.data, selectedPath])

  const save = useMutation({
    mutationFn: () => {
      if (!selectedPath || !draft.data) throw new Error("Select a file before saving.")
      return api.saveResourceDraftFile(
        resourceId,
        selectedPath,
        editorValue,
        draft.data.revision,
      )
    },
    onSuccess: (tree) => {
      queryClient.setQueryData([RESOURCE_QUERY_KEY, resourceId, "draft"], tree)
      setDirty(false)
      setValidation(null)
      void queryClient.invalidateQueries({ queryKey: [RESOURCE_QUERY_KEY] })
    },
  })

  const validate = useMutation({
    mutationFn: () => api.validateResourceDraft(resourceId),
    onSuccess: setValidation,
  })

  const importArchive = useMutation({
    mutationFn: (file: File) => {
      if (!draft.data) throw new Error("The current draft must finish loading first.")
      return api.importResourceDraft(resourceId, file, draft.data.revision)
    },
    onSuccess: ({ tree, validation: result }) => {
      queryClient.setQueryData([RESOURCE_QUERY_KEY, resourceId, "draft"], tree)
      setValidation(result)
      const first = tree.files[0]
      setSelectedPath(first?.path ?? null)
      setEditorValue(first?.content ?? "")
      setDirty(false)
      setReleaseResult(
        `Imported ${tree.files.length} editable files at revision ${tree.revision}. ${
          result.valid ? "Static validation passed." : "Review the diagnostics before release."
        }`,
      )
      void queryClient.invalidateQueries({ queryKey: [RESOURCE_QUERY_KEY] })
    },
  })

  function selectFile(path: string, tree: DraftFileTree) {
    if (dirty && !window.confirm("Discard the unsaved editor change?")) return
    const file = tree.files.find((item) => item.path === path)
    if (!file) return
    setSelectedPath(path)
    setEditorValue(file.content)
    setDirty(false)
  }

  if (resources.isLoading) {
    return (
      <PageFrame title="Resource Studio" subtitle="Loading governed source…">
        <SkeletonRows rows={8} />
      </PageFrame>
    )
  }
  if (resources.error || !resource) {
    return (
      <PageFrame title="Resource Studio" subtitle="Governed source editor">
        <ErrorState
          message={
            resources.error instanceof Error
              ? resources.error.message
              : "This resource does not exist or is outside your project."
          }
        />
      </PageFrame>
    )
  }

  return (
    <PageFrame
      title={resource.name}
      subtitle={`${RESOURCE_KIND_LABEL[resource.kind]} · ${resource.slug} · project ${resource.project_id.slice(0, 8)}`}
      action={
        <div className="flex flex-wrap gap-2">
          <Button variant="outline" onClick={() => void navigate({ to: "/app/resources" })}>
            <ArrowLeft className="size-3.5" />
            Catalog
          </Button>
          <Button
            variant="outline"
            disabled={validate.isPending || dirty}
            onClick={() => validate.mutate()}
          >
            <ShieldCheck className="size-3.5" />
            {validate.isPending ? "Validating…" : "Validate"}
          </Button>
          <input
            ref={importInput}
            type="file"
            accept={RESOURCE_IMPORT_ACCEPT}
            className="hidden"
            data-testid="resource-zip-input"
            onChange={(event) => {
              const file = event.target.files?.[0]
              event.target.value = ""
              if (!file) return
              if (
                window.confirm(
                  "Replace every file in the current Draft with this ZIP? Published releases stay immutable.",
                )
              ) {
                importArchive.mutate(file)
              }
            }}
          />
          <Button
            variant="outline"
            disabled={dirty || draft.isLoading || importArchive.isPending}
            onClick={() => importInput.current?.click()}
          >
            <Upload className="size-3.5" />
            {importArchive.isPending ? "Importing…" : "Import ZIP"}
          </Button>
          <Button
            variant="gradient"
            disabled={dirty || draft.isLoading || resource.status === "archived"}
            onClick={() => setShowRelease(true)}
          >
            <Rocket className="size-3.5" />
            Release
          </Button>
        </div>
      }
    >
      <div className="mb-4 grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
        <StudioMetric
          label="Lifecycle"
          value={resource.status}
          icon={CircleDot}
          tone={resource.status === "published" ? "success" : "warning"}
        />
        <StudioMetric
          label="Current channel"
          value={resource.release_channel ?? "Unreleased"}
          icon={GitBranch}
          tone="accent"
        />
        <StudioMetric
          label="Highest version"
          value={resource.highest_version ? `v${resource.highest_version}` : "First: v0.1.0"}
          icon={PackageCheck}
          tone="neutral"
        />
        <StudioMetric
          label="Draft revision"
          value={`#${draft.data?.revision ?? resource.draft_revision}`}
          icon={FileCode2}
          tone={dirty ? "warning" : "neutral"}
        />
      </div>

      <div className="mb-4 flex gap-1 border-b border-(--border-soft)">
        {([
          ["studio", "Source & validation"],
          ["releases", `Versions (${versions.data?.length ?? 0})`],
        ] as const).map(([value, label]) => (
          <button
            key={value}
            type="button"
            onClick={() => setTab(value)}
            className={`border-b-2 px-3 py-2 text-xs font-medium transition-colors ${
              tab === value
                ? "border-(--color-accent) text-(--color-text)"
                : "border-transparent text-(--color-text-muted) hover:text-(--color-text)"
            }`}
          >
            {label}
          </button>
        ))}
      </div>

      {releaseResult && (
        <div className="mb-4 flex items-center gap-2 rounded-lg border border-(--color-success)/30 bg-(--color-success)/8 px-3 py-2 text-xs text-(--color-success)">
          <CheckCircle2 className="size-4" />
          {releaseResult}
        </div>
      )}

      {tab === "studio" ? (
        <StudioWorkspace
          resource={resource}
          draft={draft.data}
          loading={draft.isLoading}
          error={draft.error ?? save.error ?? validate.error ?? importArchive.error}
          selectedPath={selectedPath}
          editorValue={editorValue}
          dirty={dirty}
          saving={save.isPending}
          validation={validation}
          onSelectFile={selectFile}
          onChange={(value) => {
            setEditorValue(value)
            setDirty(true)
          }}
          onSave={() => save.mutate()}
        />
      ) : (
        <ReleaseHistory resource={resource} loading={versions.isLoading} versions={versions.data} />
      )}

      <ReleaseDialog
        open={showRelease}
        resource={resource}
        draftRevision={draft.data?.revision ?? resource.draft_revision}
        onClose={() => setShowRelease(false)}
        onReleased={(message) => {
          setShowRelease(false)
          setReleaseResult(message)
          setValidation(null)
          void queryClient.invalidateQueries({ queryKey: [RESOURCE_QUERY_KEY] })
          void queryClient.invalidateQueries({
            queryKey: [RESOURCE_QUERY_KEY, resourceId, "versions"],
          })
          void queryClient.invalidateQueries({
            queryKey: [RESOURCE_QUERY_KEY, resourceId, "draft"],
          })
        }}
      />
    </PageFrame>
  )
}

function StudioWorkspace({
  resource,
  draft,
  loading,
  error,
  selectedPath,
  editorValue,
  dirty,
  saving,
  validation,
  onSelectFile,
  onChange,
  onSave,
}: {
  resource: ManagedResource
  draft?: DraftFileTree
  loading: boolean
  error: unknown
  selectedPath: string | null
  editorValue: string
  dirty: boolean
  saving: boolean
  validation: ResourceValidation | null
  onSelectFile: (path: string, tree: DraftFileTree) => void
  onChange: (value: string) => void
  onSave: () => void
}) {
  if (loading) return <SkeletonRows rows={8} />
  if (error) {
    return <ErrorState message={error instanceof Error ? error.message : "Resource Studio failed"} />
  }
  if (!draft?.files.length) {
    return (
      <EmptyState
        icon={FileCode2}
        title="Draft has no files"
        description="Create this resource again from its starter template or import a valid package."
      />
    )
  }
  return (
    <div
      className="grid min-h-[620px] overflow-hidden rounded-xl border border-(--border-card) bg-(--bg-card) lg:grid-cols-[220px_minmax(0,1fr)_290px]"
      data-testid="resource-studio"
    >
      <aside className="border-b border-(--border-soft) lg:border-r lg:border-b-0">
        <div className="border-b border-(--border-soft) px-3 py-3">
          <div className="text-[0.68rem] font-semibold tracking-wider text-(--color-text-subtle) uppercase">
            Draft files
          </div>
          <div className="mt-1 text-xs text-(--color-text-muted)">
            {draft.files.length} text {draft.files.length === 1 ? "file" : "files"}
          </div>
        </div>
        <div className="max-h-56 overflow-auto p-2 lg:max-h-[560px]">
          {draft.files.map((file) => (
            <button
              key={file.path}
              type="button"
              onClick={() => onSelectFile(file.path, draft)}
              className={`mb-1 flex w-full items-center gap-2 rounded-md px-2 py-2 text-left font-mono text-[0.7rem] transition-colors ${
                file.path === selectedPath
                  ? "bg-(--color-accent-soft) text-(--color-accent)"
                  : "text-(--color-text-muted) hover:bg-(--bg-key) hover:text-(--color-text)"
              }`}
            >
              <FileCode2 className="size-3.5 shrink-0" />
              <span className="min-w-0 truncate">{file.path}</span>
            </button>
          ))}
        </div>
      </aside>

      <section className="min-w-0 border-b border-(--border-soft) lg:border-r lg:border-b-0">
        <div className="flex min-h-12 items-center justify-between gap-3 border-b border-(--border-soft) px-3">
          <div className="min-w-0">
            <div className="truncate font-mono text-xs">{selectedPath}</div>
            <div className="text-[0.65rem] text-(--color-text-subtle)">
              {dirty ? "Unsaved changes" : `Saved at revision ${draft.revision}`}
            </div>
          </div>
          <Button size="sm" disabled={!dirty || saving} onClick={onSave}>
            <Save className="size-3.5" />
            {saving ? "Saving…" : "Save"}
          </Button>
        </div>
        <Suspense
          fallback={<div className="grid h-[548px] place-items-center text-xs">Loading editor…</div>}
        >
          <MonacoEditor
            height="548px"
            language={editorLanguage(selectedPath)}
            theme="vs-dark"
            value={editorValue}
            onChange={(value) => onChange(value ?? "")}
            options={{
              minimap: { enabled: false },
              fontFamily: "JetBrains Mono Variable, monospace",
              fontSize: 13,
              lineHeight: 21,
              scrollBeyondLastLine: false,
              wordWrap: "on",
              padding: { top: 14 },
              automaticLayout: true,
            }}
          />
        </Suspense>
      </section>

      <aside className="p-3">
        <div className="mb-4">
          <div className="flex items-center gap-2 text-xs font-medium">
            <Sparkles className="size-3.5 text-(--color-accent)" />
            {RESOURCE_KIND_LABEL[resource.kind]} guide
          </div>
          <p className="mt-2 text-xs leading-5 text-(--color-text-muted)">
            {resource.kind === RESOURCE_KIND.PLUGIN
              ? "Keep plugin.json at the root. Releases are packaged immutably and arrive disabled in EvoFlux until the member reviews commands, hosts and capabilities."
              : resource.kind === RESOURCE_KIND.SKILL
                ? "SKILL.md must contain name and description frontmatter. The saved source remains a Draft until you create a Beta or Published release."
                : "Agent Markdown must declare matching name and description frontmatter. Validation never executes source content."}
          </p>
        </div>
        <div className="mb-2 flex items-center justify-between">
          <span className="text-xs font-medium">Diagnostics</span>
          {validation && (
            <Badge tone={validation.valid ? "success" : "danger"}>
              {validation.valid ? "Valid" : `${validation.diagnostics.length} issues`}
            </Badge>
          )}
        </div>
        {!validation ? (
          <div className="rounded-lg border border-dashed border-(--color-border) p-3 text-xs leading-5 text-(--color-text-subtle)">
            Save all edits, then run Validate. Structured errors include a stable code and file path.
          </div>
        ) : validation.diagnostics.length === 0 ? (
          <div className="flex items-start gap-2 rounded-lg border border-(--color-success)/30 bg-(--color-success)/8 p-3 text-xs text-(--color-success)">
            <CheckCircle2 className="mt-0.5 size-4 shrink-0" />
            Draft passes static validation and is ready for release.
          </div>
        ) : (
          <div className="space-y-2" data-testid="resource-diagnostics">
            {validation.diagnostics.map((item, index) => (
              <div
                key={`${item.code}-${item.path}-${index}`}
                className="rounded-lg border border-(--color-error)/25 bg-(--color-error-subtle) p-2.5"
              >
                <div className="flex items-center gap-1.5 text-xs font-medium text-(--color-error)">
                  <AlertTriangle className="size-3.5" />
                  {item.code}
                </div>
                <p className="mt-1 text-[0.7rem] leading-4 text-(--color-text-muted)">
                  {item.message}
                </p>
                {item.path && (
                  <div className="mt-1.5 font-mono text-[0.65rem] text-(--color-text-subtle)">
                    {item.path}
                  </div>
                )}
              </div>
            ))}
          </div>
        )}
      </aside>
    </div>
  )
}

function ReleaseHistory({
  resource,
  loading,
  versions,
}: {
  resource: ManagedResource
  loading: boolean
  versions?: Awaited<ReturnType<typeof api.resourceVersions>>
}) {
  if (loading) return <SkeletonRows rows={6} />
  if (!versions?.length) {
    return (
      <EmptyState
        icon={GitBranch}
        title="No immutable releases yet"
        description="Validate the Draft and release v0.1.0 to Beta or Published. Saving files does not allocate a version."
      />
    )
  }
  return (
    <Card>
      <CardHeader>
        <div>
          <CardTitle>Immutable release history</CardTitle>
          <CardDescription>
            {resource.slug} · SHA-256 identifies the exact payload or Plugin archive.
          </CardDescription>
        </div>
      </CardHeader>
      <CardContent className="p-0">
        <TableWrap className="rounded-none border-0">
          <Table>
            <TableHead>
              <tr>
                <TableTh>Version</TableTh>
                <TableTh>Channel</TableTh>
                <TableTh>Integrity</TableTh>
                <TableTh>Size</TableTh>
                <TableTh>Released</TableTh>
              </tr>
            </TableHead>
            <TableBody>
              {versions.map((version) => (
                <TableRow key={version.id}>
                  <TableTd className="font-mono font-medium">v{version.version}</TableTd>
                  <TableTd>
                    <Badge tone={version.release_channel === RELEASE_CHANNEL.PUBLISHED ? "success" : "warning"}>
                      {version.release_channel ?? version.status}
                    </Badge>
                  </TableTd>
                  <TableTd className="font-mono text-[0.68rem] text-(--color-text-muted)">
                    {version.content_sha256 ? version.content_sha256.slice(0, 12) : "Legacy"}
                  </TableTd>
                  <TableTd>{formatBytes(version.content_size)}</TableTd>
                  <TableTd className="text-xs text-(--color-text-muted)">
                    {new Date(version.created_at).toLocaleString()}
                  </TableTd>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </TableWrap>
      </CardContent>
    </Card>
  )
}

function ReleaseDialog({
  open,
  resource,
  draftRevision,
  onClose,
  onReleased,
}: {
  open: boolean
  resource: ManagedResource
  draftRevision: number
  onClose: () => void
  onReleased: (message: string) => void
}) {
  const [channel, setChannel] = useState<ReleaseChannel>(RELEASE_CHANNEL.PUBLISHED)
  const [mode, setMode] = useState<VersionMode>(VERSION_MODE.AUTO)
  const [manualVersion, setManualVersion] = useState("")
  const [changelog, setChangelog] = useState("")
  const [minimumVersion, setMinimumVersion] = useState("")
  const [betaMembers, setBetaMembers] = useState<string[]>([])
  const members = useQuery({
    queryKey: ["members", "release-audience"],
    queryFn: () => api.members({ status: "active", limit: 100 }),
    enabled: open && channel === RELEASE_CHANNEL.BETA,
  })
  const release = useMutation({
    mutationFn: () => {
      const body: ReleaseResourceRequest = {
        channel,
        version_mode: mode,
        manual_version: mode === VERSION_MODE.MANUAL ? manualVersion.trim() : null,
        draft_revision: draftRevision,
        changelog: changelog.trim() || null,
        beta_member_ids: channel === RELEASE_CHANNEL.BETA ? betaMembers : [],
        minimum_evoflux_version: minimumVersion.trim() || null,
      }
      return api.releaseResource(resource.id, body)
    },
    onSuccess: (result) =>
      onReleased(
        `Released v${result.version} to ${result.channel}. Integrity ${result.sha256.slice(0, 12)}…`,
      ),
  })
  const preview = mode === VERSION_MODE.AUTO ? nextPatch(resource.highest_version) : manualVersion || "—"
  return (
    <Dialog
      open={open}
      title={`Release ${resource.name}`}
      description="Creates immutable content and atomically moves the selected channel."
      onClose={onClose}
      className="sm:max-w-2xl"
      footer={
        <>
          <Button variant="ghost" disabled={release.isPending} onClick={onClose}>
            Cancel
          </Button>
          <Button
            variant="gradient"
            disabled={
              release.isPending ||
              (mode === VERSION_MODE.MANUAL && !manualVersion.trim()) ||
              (channel === RELEASE_CHANNEL.BETA && betaMembers.length === 0)
            }
            onClick={() => release.mutate()}
          >
            <Rocket className="size-3.5" />
            {release.isPending ? "Releasing…" : `Release v${preview}`}
          </Button>
        </>
      }
    >
      {release.error && (
        <ErrorState
          className="mb-4"
          message={release.error instanceof Error ? release.error.message : "Release failed"}
        />
      )}
      <div className="mb-4 grid grid-cols-3 gap-2 rounded-xl border border-(--border-card) bg-(--bg-key) p-3 text-center">
        <div>
          <div className="text-[0.65rem] text-(--color-text-subtle)">Current head</div>
          <div className="mt-1 font-mono text-sm">{resource.highest_version ?? "None"}</div>
        </div>
        <div className="grid place-items-center">
          <ChevronRight className="size-4 text-(--color-text-subtle)" />
        </div>
        <div>
          <div className="text-[0.65rem] text-(--color-text-subtle)">Allocated</div>
          <div className="mt-1 font-mono text-sm text-(--color-accent)">v{preview}</div>
        </div>
      </div>
      <div className="grid gap-4 sm:grid-cols-2">
        <div>
          <Label htmlFor="release-channel">Channel</Label>
          <Select
            id="release-channel"
            className="mt-1.5"
            value={channel}
            onValueChange={setChannel}
            options={[
              { value: RELEASE_CHANNEL.PUBLISHED, label: "Published — all eligible members" },
              { value: RELEASE_CHANNEL.BETA, label: "Beta — selected eligible members" },
            ]}
          />
        </div>
        <div>
          <Label htmlFor="version-mode">Version allocation</Label>
          <Select
            id="version-mode"
            className="mt-1.5"
            value={mode}
            onValueChange={setMode}
            options={[
              { value: VERSION_MODE.AUTO, label: "Auto — next patch" },
              { value: VERSION_MODE.MANUAL, label: "Manual — validate SemVer" },
            ]}
          />
        </div>
        {mode === VERSION_MODE.MANUAL && (
          <div>
            <Label htmlFor="manual-version">Manual version</Label>
            <Input
              id="manual-version"
              className="mt-1.5 font-mono"
              value={manualVersion}
              onChange={(event) => setManualVersion(event.target.value)}
              placeholder="1.0.0"
            />
          </div>
        )}
        <div>
          <Label htmlFor="minimum-evoflux">Minimum EvoFlux version</Label>
          <Input
            id="minimum-evoflux"
            className="mt-1.5 font-mono"
            value={minimumVersion}
            onChange={(event) => setMinimumVersion(event.target.value)}
            placeholder="Optional, e.g. 0.9.0"
          />
        </div>
        {channel === RELEASE_CHANNEL.BETA && (
          <div className="sm:col-span-2">
            <Label htmlFor="beta-members">Beta members</Label>
            <MultiSelect
              id="beta-members"
              className="mt-1.5"
              value={betaMembers}
              onChange={setBetaMembers}
              options={(members.data?.items ?? []).map((member) => ({
                value: member.id,
                label: `${member.display_name} · ${member.email}`,
              }))}
            />
            <p className="mt-1.5 flex items-center gap-1 text-[0.68rem] text-(--color-text-subtle)">
              <Users className="size-3" /> Beta targeting narrows access; it never grants project access.
            </p>
          </div>
        )}
        <div className="sm:col-span-2">
          <Label htmlFor="release-changelog">Changelog</Label>
          <Textarea
            id="release-changelog"
            className="mt-1.5 min-h-24"
            value={changelog}
            onChange={(event) => setChangelog(event.target.value)}
            placeholder="What changed in this immutable release?"
          />
        </div>
      </div>
    </Dialog>
  )
}

function StudioMetric({
  label,
  value,
  icon: Icon,
  tone,
}: {
  label: string
  value: string
  icon: typeof FileCode2
  tone: "success" | "warning" | "accent" | "neutral"
}) {
  const tones = {
    success: "bg-(--color-success)/10 text-(--color-success)",
    warning: "bg-(--color-warning)/10 text-(--color-warning)",
    accent: "bg-(--color-accent-soft) text-(--color-accent)",
    neutral: "bg-(--bg-key) text-(--color-text-muted)",
  }
  return (
    <Card>
      <CardContent className="flex items-center gap-3 p-3">
        <span className={`grid size-9 shrink-0 place-items-center rounded-lg ${tones[tone]}`}>
          <Icon className="size-4" />
        </span>
        <div className="min-w-0">
          <div className="truncate text-sm font-semibold capitalize">{value}</div>
          <div className="text-[0.68rem] text-(--color-text-muted)">{label}</div>
        </div>
      </CardContent>
    </Card>
  )
}

function editorLanguage(path: string | null) {
  if (path?.endsWith(".json")) return "json"
  if (path?.endsWith(".yaml") || path?.endsWith(".yml")) return "yaml"
  if (path?.endsWith(".py")) return "python"
  if (path?.endsWith(".ts") || path?.endsWith(".tsx")) return "typescript"
  return "markdown"
}

function nextPatch(highest: string | null) {
  if (!highest) return "0.1.0"
  const stable = highest.split(/[+-]/, 1)[0]
  const parts = stable.split(".").map(Number)
  if (parts.length !== 3 || parts.some(Number.isNaN)) return "server allocated"
  if (highest.includes("-")) return stable
  return `${parts[0]}.${parts[1]}.${parts[2] + 1}`
}

function formatBytes(bytes: number) {
  if (!bytes) return "—"
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KiB`
  return `${(bytes / 1024 / 1024).toFixed(1)} MiB`
}
