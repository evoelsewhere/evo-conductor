import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"
import { useNavigate, useParams } from "@tanstack/react-router"
import {
  ArrowLeft,
  CheckCircle2,
  ChevronRight,
  Rocket,
  ShieldCheck,
  Upload,
  Users,
} from "lucide-react"
import { useEffect, useRef, useState } from "react"

import {
  api,
  type ManagedResource,
  type ReleaseResourceRequest,
  type ResourceValidation,
} from "@/shared/api/client"
import {
  RELEASE_CHANNEL,
  RESOURCE_IMPORT_ACCEPT,
  RESOURCE_KIND,
  RESOURCE_KIND_LABEL,
  RESOURCE_MODE_SCOPE_FILENAME,
  RESOURCE_QUERY_KEY,
  RESOURCE_TARGET_MODES,
  VERSION_MODE,
  type ReleaseChannel,
  type ResourceTargetMode,
  type VersionMode,
} from "@/shared/constants/resource"
import {
  RESOURCE_STUDIO_TAB,
  resourceStudioInitialContent,
  type ResourceStudioTab,
} from "@/shared/constants/resource-studio"
import { RESOURCE_KIND_USAGE_PATHS } from "@/shared/constants/resource-monitoring"
import { PageFrame } from "@/shared/components/page-frame"
import {
  PERMISSION,
  bestAuthorizationDecision,
  mayRequest,
} from "@/shared/lib/authorization"
import { useAuthStore } from "@/shared/stores/auth"
import { ResourceStudioWorkbench } from "@/features/resources/components/resource-studio-workbench"
import { ResourceDetailMonitoring } from "@/features/resources/components/resource-detail-monitoring"
import { ResourceModeSelector } from "@/features/resources/components/resource-mode-selector"
import { ResourceVersionHistory } from "@/features/resources/components/resource-version-history"
import { Button } from "@/shared/ui/button"
import { Dialog } from "@/shared/ui/dialog"
import { ErrorState } from "@/shared/ui/empty-state"
import { Input } from "@/shared/ui/input"
import { Label } from "@/shared/ui/label"
import { MultiSelect } from "@/shared/ui/multi-select"
import { Select } from "@/shared/ui/select"
import { SkeletonRows } from "@/shared/ui/skeleton"
import { Textarea } from "@/shared/ui/textarea"

export function ResourceStudioPage() {
  const { resourceId } = useParams({ strict: false }) as {
    resourceId: string
  }
  const navigate = useNavigate()
  const queryClient = useQueryClient()
  const can = useAuthStore((state) => state.can)
  const [tab, setTab] = useState<ResourceStudioTab>(RESOURCE_STUDIO_TAB.SOURCE)
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
  const permissionTarget = resource
    ? {
        ownerId: resource.owner_user_id,
        resourceKind: resource.kind,
        lifecycle: resource.status,
      }
    : undefined
  const canAuthor = Boolean(
    resource && mayRequest(can(PERMISSION.RESOURCE_AUTHOR, permissionTarget)),
  )
  const canRelease = Boolean(
    resource &&
      mayRequest(
        bestAuthorizationDecision([
          can(PERMISSION.RESOURCE_RELEASE_NON_EXECUTABLE, permissionTarget),
          can(PERMISSION.RESOURCE_RELEASE_RESTRICTED, permissionTarget),
        ]),
      ),
  )
  const canManageLifecycle = Boolean(
    resource && mayRequest(can(PERMISSION.RESOURCE_LIFECYCLE_MANAGE, permissionTarget)),
  )
  const canMonitor = Boolean(
    resource &&
      mayRequest(
        bestAuthorizationDecision([
          can(PERMISSION.RESOURCE_MONITORING_AGGREGATE_READ, permissionTarget),
          can(PERMISSION.RESOURCE_MONITORING_MEMBER_DETAIL_READ, permissionTarget),
        ]),
      ),
  )
  const canReadMemberMonitoring = Boolean(
    resource &&
      mayRequest(
        can(PERMISSION.RESOURCE_MONITORING_MEMBER_DETAIL_READ, permissionTarget),
      ),
  )
  const draft = useQuery({
    queryKey: [RESOURCE_QUERY_KEY, resourceId, "draft"],
    queryFn: () => api.resourceDraft(resourceId),
    enabled: Boolean(resource && canAuthor),
  })
  const versions = useQuery({
    queryKey: [RESOURCE_QUERY_KEY, resourceId, "versions"],
    queryFn: () => api.resourceVersions(resourceId),
    enabled: Boolean(resource && canAuthor),
  })
  const targetModes = resource && ["agent", "skill"].includes(resource.kind)
    ? modesFromDraft(draft.data?.files)
    : null

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

  const saveTargetModes = useMutation({
    mutationFn: (modes: ResourceTargetMode[]) => {
      if (!draft.data) throw new Error("The current draft must finish loading first.")
      return api.saveResourceDraftFile(
        resourceId,
        RESOURCE_MODE_SCOPE_FILENAME,
        `${JSON.stringify({ modes }, null, 2)}\n`,
        draft.data.revision,
      )
    },
    onSuccess: (tree) => {
      queryClient.setQueryData([RESOURCE_QUERY_KEY, resourceId, "draft"], tree)
      setValidation(null)
      void queryClient.invalidateQueries({ queryKey: [RESOURCE_QUERY_KEY] })
    },
  })

  const createFile = useMutation({
    mutationFn: (path: string) => {
      if (!draft.data) throw new Error("The current draft must finish loading first.")
      return api.createResourceDraftFile(
        resourceId,
        path,
        resourceStudioInitialContent(path),
        draft.data.revision,
      )
    },
    onSuccess: (tree, path) => {
      queryClient.setQueryData([RESOURCE_QUERY_KEY, resourceId, "draft"], tree)
      const file = tree.files.find((item) => item.path === path)
      setSelectedPath(file?.path ?? tree.files[0]?.path ?? null)
      setEditorValue(file?.content ?? tree.files[0]?.content ?? "")
      setDirty(false)
      setValidation(null)
      void queryClient.invalidateQueries({ queryKey: [RESOURCE_QUERY_KEY] })
    },
  })

  const moveEntry = useMutation({
    mutationFn: ({ path, destinationPath }: { path: string; destinationPath: string }) => {
      if (!draft.data) throw new Error("The current draft must finish loading first.")
      return api.moveResourceDraftEntry(
        resourceId,
        path,
        destinationPath,
        draft.data.revision,
      )
    },
    onSuccess: (tree, { path, destinationPath }) => {
      queryClient.setQueryData([RESOURCE_QUERY_KEY, resourceId, "draft"], tree)
      const sourcePrefix = `${path}/`
      const nextSelected = selectedPath === path
        ? destinationPath
        : selectedPath?.startsWith(sourcePrefix)
          ? `${destinationPath}/${selectedPath.slice(sourcePrefix.length)}`
          : selectedPath
      const file = tree.files.find((item) => item.path === nextSelected) ?? tree.files[0]
      setSelectedPath(file?.path ?? null)
      setEditorValue(file?.content ?? "")
      setDirty(false)
      setValidation(null)
      void queryClient.invalidateQueries({ queryKey: [RESOURCE_QUERY_KEY] })
    },
  })

  const deleteEntry = useMutation({
    mutationFn: (path: string) => {
      if (!draft.data) throw new Error("The current draft must finish loading first.")
      return api.deleteResourceDraftEntry(resourceId, path, draft.data.revision)
    },
    onSuccess: (tree) => {
      queryClient.setQueryData([RESOURCE_QUERY_KEY, resourceId, "draft"], tree)
      const file = tree.files.find((item) => item.path === selectedPath) ?? tree.files[0]
      setSelectedPath(file?.path ?? null)
      setEditorValue(file?.content ?? "")
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

  function selectFile(path: string) {
    if (dirty && !window.confirm("Discard the unsaved editor change?")) return
    const file = draft.data?.files.find((item) => item.path === path)
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
  if (!canAuthor) {
    return (
      <PageFrame title="Resource Studio" subtitle="Governed source editor">
        <ErrorState message="Forbidden. Only a project admin or the owning Contributor can edit this resource." />
      </PageFrame>
    )
  }

  return (
    <PageFrame
      title={resource.name}
      subtitle={`${RESOURCE_KIND_LABEL[resource.kind]} · ${resource.slug} · project ${resource.project_id.slice(0, 8)}`}
      className="max-w-none"
      action={
        <div className="flex flex-wrap gap-2">
          <Button
            variant="outline"
            onClick={() => {
              if (!dirty || window.confirm("Discard the unsaved editor change?")) {
                void navigate({ to: resourceCatalogPath(resource.kind) })
              }
            }}
          >
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
          {canRelease && (
            <Button
              variant="gradient"
              disabled={dirty || draft.isLoading || resource.status === "archived"}
              onClick={() => setShowRelease(true)}
            >
              <Rocket className="size-3.5" />
              Release
            </Button>
          )}
        </div>
      }
    >
      <div className="mb-4 flex gap-1 border-b border-(--border-soft)">
        {([
          [RESOURCE_STUDIO_TAB.SOURCE, "Source & validation"],
          [RESOURCE_STUDIO_TAB.VERSIONS, `Versions (${versions.data?.length ?? 0})`],
          [RESOURCE_STUDIO_TAB.MONITORING, "Monitoring"],
        ] as const)
          .filter(([value]) => value !== RESOURCE_STUDIO_TAB.MONITORING || canMonitor)
          .map(([value, label]) => (
          <button
            key={value}
            type="button"
            onClick={() => {
              if (tab === value) return
              if (!dirty || window.confirm("Discard the unsaved editor change?")) setTab(value)
            }}
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

      {tab === RESOURCE_STUDIO_TAB.SOURCE ? (
        <>
          {targetModes && (
            <section className="mb-4 rounded-xl border border-(--border-card) bg-(--bg-card) p-4">
              <div className="mb-3">
                <h2 className="text-sm font-semibold text-(--color-text)">
                  EvoFlux availability
                </h2>
                <p className="mt-0.5 text-xs text-(--color-text-muted)">
                  Work is the cowork surface; Coding is repository-scoped. EvoFlux keeps
                  its built-in capabilities and mounts this managed resource only in the
                  selected modes.
                </p>
              </div>
              <ResourceModeSelector
                value={targetModes}
                onChange={(modes) => saveTargetModes.mutate(modes)}
                disabled={dirty || saveTargetModes.isPending || draft.isFetching}
              />
              {saveTargetModes.error && (
                <ErrorState
                  className="mt-3"
                  message={
                    saveTargetModes.error instanceof Error
                      ? saveTargetModes.error.message
                      : "Mode update failed"
                  }
                />
              )}
            </section>
          )}
          <ResourceStudioWorkbench
          resource={resource}
          draft={draft.data}
          loading={draft.isLoading}
          loadError={draft.error}
          actionError={
            save.error ??
            validate.error ??
            importArchive.error ??
            createFile.error ??
            moveEntry.error ??
            deleteEntry.error
          }
          selectedPath={selectedPath}
          editorValue={editorValue}
          dirty={dirty}
          saving={save.isPending}
          busyAction={
            createFile.isPending ||
            moveEntry.isPending ||
            deleteEntry.isPending ||
            draft.isFetching
          }
          validation={validation}
          onSelectFile={selectFile}
          onChange={(value) => {
            setEditorValue(value)
            const savedValue =
              draft.data?.files.find((file) => file.path === selectedPath)?.content ?? ""
            setDirty(value !== savedValue)
          }}
          onDiscard={() => {
            const content = draft.data?.files.find((file) => file.path === selectedPath)?.content ?? ""
            setEditorValue(content)
            setDirty(false)
          }}
          onSave={() => save.mutate()}
          onRefresh={() => {
            if (dirty && !window.confirm("Discard the unsaved editor change?")) return
            void draft.refetch()
          }}
          onCreateFile={(path) => createFile.mutateAsync(path).then(() => undefined)}
          onMoveEntry={(path, destinationPath) =>
            moveEntry.mutateAsync({ path, destinationPath }).then(() => undefined)
          }
          onDeleteEntry={(path) => deleteEntry.mutateAsync(path).then(() => undefined)}
          />
        </>
      ) : tab === RESOURCE_STUDIO_TAB.VERSIONS ? (
        <ResourceVersionHistory
          resource={resource}
          draftRevision={draft.data?.revision ?? resource.draft_revision}
          loading={versions.isLoading}
          versions={versions.data}
          canManageLifecycle={canManageLifecycle}
          onVersionDeprecated={(version) => {
            setReleaseResult(`Deprecated v${version.version}. Its immutable history remains available.`)
            void queryClient.invalidateQueries({
              queryKey: [RESOURCE_QUERY_KEY, resourceId, "versions"],
            })
          }}
          onDraftRestored={(tree, version) => {
            queryClient.setQueryData([RESOURCE_QUERY_KEY, resourceId, "draft"], tree)
            const first = tree.files[0]
            setSelectedPath(first?.path ?? null)
            setEditorValue(first?.content ?? "")
            setDirty(false)
            setValidation(null)
            setTab(RESOURCE_STUDIO_TAB.SOURCE)
            setReleaseResult(
              `Restored v${version.version} into Draft revision ${tree.revision}. Release creates a new greater version.`,
            )
            void queryClient.invalidateQueries({ queryKey: [RESOURCE_QUERY_KEY] })
          }}
        />
      ) : (
        canMonitor ? (
          <ResourceDetailMonitoring
            resource={resource}
            showMemberDetail={canReadMemberMonitoring}
          />
        ) : (
          <ErrorState message="Resource monitoring is not available for your current permissions." />
        )
      )}

      <ReleaseDialog
        open={showRelease && canRelease}
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

function modesFromDraft(
  files: { path: string; content: string }[] | undefined,
): ResourceTargetMode[] {
  const source = files?.find((file) => file.path === RESOURCE_MODE_SCOPE_FILENAME)?.content
  if (!source) return [...RESOURCE_TARGET_MODES]
  try {
    const value = JSON.parse(source) as { modes?: unknown }
    const rawModes = value.modes
    if (!Array.isArray(rawModes)) return [...RESOURCE_TARGET_MODES]
    const selected = RESOURCE_TARGET_MODES.filter((mode) => rawModes.includes(mode))
    return selected.length ? selected : [...RESOURCE_TARGET_MODES]
  } catch {
    return [...RESOURCE_TARGET_MODES]
  }
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
  const authorization = useAuthStore((state) => state.authorization)
  const can = useAuthStore((state) => state.can)
  const canReadMemberPrivate = mayRequest(
    can(PERMISSION.MEMBER_PRIVATE_READ_ANY),
  )
  const [channel, setChannel] = useState<ReleaseChannel>(RELEASE_CHANNEL.PUBLISHED)
  const [mode, setMode] = useState<VersionMode>(VERSION_MODE.AUTO)
  const [manualVersion, setManualVersion] = useState("")
  const [changelog, setChangelog] = useState("")
  const [minimumVersion, setMinimumVersion] = useState("")
  const [betaMembers, setBetaMembers] = useState<string[]>([])
  const members = useQuery({
    queryKey: [
      "members",
      "release-audience",
      authorization?.current_role,
      authorization?.policy_revision,
    ],
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
                label: canReadMemberPrivate && "email" in member
                  ? `${member.display_name} · ${member.email}`
                  : member.display_name,
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

function nextPatch(highest: string | null) {
  if (!highest) return "0.1.0"
  const stable = highest.split(/[+-]/, 1)[0]
  const parts = stable.split(".").map(Number)
  if (parts.length !== 3 || parts.some(Number.isNaN)) return "server allocated"
  if (highest.includes("-")) return stable
  return `${parts[0]}.${parts[1]}.${parts[2] + 1}`
}

function resourceCatalogPath(kind: ManagedResource["kind"]) {
  if (kind === RESOURCE_KIND.PLUGIN) return RESOURCE_KIND_USAGE_PATHS.plugin.overview
  if (kind === RESOURCE_KIND.SKILL) return RESOURCE_KIND_USAGE_PATHS.skill.overview
  if (kind === RESOURCE_KIND.AGENT) return RESOURCE_KIND_USAGE_PATHS.agent.overview
  return "/app/resources" as const
}
