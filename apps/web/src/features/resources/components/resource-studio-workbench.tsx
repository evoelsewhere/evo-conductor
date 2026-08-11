import { useMonaco, type OnMount } from "@monaco-editor/react"
import type { editor as MonacoEditorApi } from "monaco-editor"
import {
  AlertTriangle,
  Check,
  CheckCircle2,
  ChevronRight,
  Copy,
  Download,
  Eye,
  FilePlus2,
  FileText,
  GitCompare,
  Info,
  Loader2,
  Maximize2,
  Minimize2,
  PanelRightClose,
  PanelRightOpen,
  Pencil,
  RefreshCw,
  Save,
  Search,
  Sparkles,
  Trash2,
  Undo2,
  X,
} from "lucide-react"
import {
  lazy,
  Suspense,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react"

import type {
  DraftFile,
  DraftFileTree,
  ManagedResource,
  ResourceDiagnostic,
  ResourceValidation,
} from "@/shared/api/client"
import { FileTypeIcon, FolderTypeIcon } from "@/shared/components/file-type-icon"
import {
  resourceLineDiff,
  type ResourceLineDiff,
} from "@/features/resources/lib/resource-line-diff"
import {
  RESOURCE_STUDIO_CHANGE_KIND,
  RESOURCE_STUDIO_DEFAULT_NEW_FILE,
  RESOURCE_STUDIO_ENTRY_KIND,
  RESOURCE_STUDIO_LAYOUT,
  RESOURCE_STUDIO_PANEL,
  RESOURCE_STUDIO_TREE,
  RESOURCE_STUDIO_VIEW_MODE,
  resourceStudioCanPreview,
  resourceStudioExtension,
  resourceStudioLanguage,
  resourceStudioRequiredEntry,
  type ResourceStudioEntryKind,
  type ResourceStudioPanel,
  type ResourceStudioViewMode,
} from "@/shared/constants/resource-studio"
import { RESOURCE_KIND, RESOURCE_KIND_LABEL } from "@/shared/constants/resource"
import { useIsDesktop } from "@/shared/hooks/use-media-query"
import { cn } from "@/shared/lib/utils"
import { useThemeStore } from "@/shared/stores/theme"
import { Badge } from "@/shared/ui/badge"
import { Button } from "@/shared/ui/button"
import { EmptyState, ErrorState } from "@/shared/ui/empty-state"
import { Input } from "@/shared/ui/input"
import { SkeletonRows } from "@/shared/ui/skeleton"

const MonacoEditor = lazy(() => import("@/shared/components/code-editor"))
const MonacoDiffEditor = lazy(() =>
  import("@/shared/components/code-editor").then((module) => ({
    default: module.CodeDiffEditor,
  })),
)

type TreeNode = {
  name: string
  path: string
  file?: DraftFile
  children: Map<string, TreeNode>
}

type EntryEditor = {
  action: "create" | "rename"
  path: string
}

const RESOURCE_TREE_NAME_COLLATOR = new Intl.Collator(undefined, {
  numeric: true,
  sensitivity: "base",
})

export function ResourceStudioWorkbench({
  resource,
  draft,
  loading,
  loadError,
  actionError,
  selectedPath,
  editorValue,
  dirty,
  saving,
  busyAction,
  validation,
  onSelectFile,
  onChange,
  onDiscard,
  onSave,
  onRefresh,
  onCreateFile,
  onMoveEntry,
  onDeleteEntry,
}: {
  resource: ManagedResource
  draft?: DraftFileTree
  loading: boolean
  loadError: unknown
  actionError: unknown
  selectedPath: string | null
  editorValue: string
  dirty: boolean
  saving: boolean
  busyAction: boolean
  validation: ResourceValidation | null
  onSelectFile: (path: string) => void
  onChange: (value: string) => void
  onDiscard: () => void
  onSave: () => void
  onRefresh: () => void
  onCreateFile: (path: string) => Promise<void>
  onMoveEntry: (path: string, destinationPath: string) => Promise<void>
  onDeleteEntry: (path: string) => Promise<void>
}) {
  const isDesktop = useIsDesktop()
  const monaco = useMonaco()
  const resolvedTheme = useThemeStore((state) => state.resolved)
  const rootRef = useRef<HTMLDivElement>(null)
  const searchRef = useRef<HTMLInputElement>(null)
  const editorRef = useRef<Parameters<OnMount>[0] | null>(null)
  const [editorInstance, setEditorInstance] = useState<Parameters<OnMount>[0] | null>(
    null,
  )
  const [viewMode, setViewMode] = useState<ResourceStudioViewMode>(
    RESOURCE_STUDIO_VIEW_MODE.FILE,
  )
  const [activePanel, setActivePanel] = useState<ResourceStudioPanel>(
    RESOURCE_STUDIO_PANEL.FILES,
  )
  const [treeVisible, setTreeVisible] = useState(true)
  const [expanded, setExpanded] = useState(false)
  const [query, setQuery] = useState("")
  const [focusedPath, setFocusedPath] = useState<string | null>(selectedPath)
  const [focusedKind, setFocusedKind] = useState<ResourceStudioEntryKind>(
    RESOURCE_STUDIO_ENTRY_KIND.FILE,
  )
  const [expandedDirectories, setExpandedDirectories] = useState<Set<string>>(new Set())
  const [entryEditor, setEntryEditor] = useState<EntryEditor | null>(null)
  const [copied, setCopied] = useState(false)
  const [cursor, setCursor] = useState({ line: 1, column: 1 })
  const [pendingReveal, setPendingReveal] = useState<{ path: string; line: number } | null>(
    null,
  )
  const [treeWidth, setTreeWidth] = useState(() => readTreeWidth())

  const selectedFile = draft?.files.find((file) => file.path === selectedPath)
  const savedContent = selectedFile?.content ?? ""
  const canPreview = resourceStudioCanPreview(selectedPath)
  const requiredEntry = resourceStudioRequiredEntry(resource.kind, resource.slug)
  const focusedIsProtected =
    focusedPath === requiredEntry ||
    (focusedPath ? requiredEntry.startsWith(`${focusedPath}/`) : false)
  const diagnostics = validation?.diagnostics ?? []
  const errorCount = diagnostics.filter((item) => item.severity === "error").length
  const warningCount = diagnostics.length - errorCount
  const actionErrorMessage = actionError instanceof Error ? actionError.message : null
  const monacoTheme = resolvedTheme === "dark" ? "vs-dark" : "light"
  const lineDiff = useMemo(
    () => resourceLineDiff(savedContent, editorValue),
    [editorValue, savedContent],
  )

  const visibleFiles = useMemo(() => {
    const normalized = query.trim().toLowerCase()
    if (!normalized) return draft?.files ?? []
    return (draft?.files ?? []).filter((file) => file.path.toLowerCase().includes(normalized))
  }, [draft?.files, query])
  const tree = useMemo(() => buildTree(visibleFiles), [visibleFiles])

  useEffect(() => {
    const directories = directoryPaths(draft?.files ?? [])
    setExpandedDirectories((previous) =>
      previous.size === 0 ? new Set(directories) : new Set([...previous, ...parentPaths(selectedPath)]),
    )
  }, [draft?.files, selectedPath])

  useEffect(() => {
    setFocusedPath(selectedPath)
    setFocusedKind(RESOURCE_STUDIO_ENTRY_KIND.FILE)
    setViewMode(
      pendingReveal?.path === selectedPath
        ? RESOURCE_STUDIO_VIEW_MODE.EDIT
        : RESOURCE_STUDIO_VIEW_MODE.FILE,
    )
  }, [pendingReveal?.path, selectedPath])

  useEffect(() => {
    if (!dirty) return
    const preventUnload = (event: BeforeUnloadEvent) => event.preventDefault()
    window.addEventListener("beforeunload", preventUnload)
    return () => window.removeEventListener("beforeunload", preventUnload)
  }, [dirty])

  useEffect(() => {
    if (!expanded) return
    const previousOverflow = document.body.style.overflow
    document.body.style.overflow = "hidden"
    return () => {
      document.body.style.overflow = previousOverflow
    }
  }, [expanded])

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "s") {
        event.preventDefault()
        if (dirty && !saving) onSave()
      }
      if (event.key === "Escape" && expanded) setExpanded(false)
    }
    window.addEventListener("keydown", onKeyDown)
    return () => window.removeEventListener("keydown", onKeyDown)
  }, [dirty, expanded, onSave, saving])

  useEffect(() => {
    if (!pendingReveal || pendingReveal.path !== selectedPath || !editorRef.current) return
    editorRef.current.revealLineInCenter(pendingReveal.line)
    editorRef.current.setPosition({ lineNumber: pendingReveal.line, column: 1 })
    editorRef.current.focus()
    setPendingReveal(null)
  }, [pendingReveal, selectedPath, viewMode])

  useEffect(() => {
    if (
      !monaco ||
      !editorInstance ||
      !dirty ||
      viewMode !== RESOURCE_STUDIO_VIEW_MODE.EDIT
    ) {
      return
    }

    const styles = getComputedStyle(document.documentElement)
    const colorByKind = {
      [RESOURCE_STUDIO_CHANGE_KIND.MODIFIED]: styles
        .getPropertyValue("--color-change")
        .trim(),
      [RESOURCE_STUDIO_CHANGE_KIND.ADDED]: styles
        .getPropertyValue("--color-success")
        .trim(),
      [RESOURCE_STUDIO_CHANGE_KIND.DELETED]: styles
        .getPropertyValue("--color-error")
        .trim(),
    }
    const decorations: MonacoEditorApi.IModelDeltaDecoration[] = lineDiff.ranges.map(
      (change) => ({
        range: new monaco.Range(change.startLine, 1, change.endLine, 1),
        options: {
          isWholeLine: true,
          className: `resource-studio-change-line-${change.kind}`,
          linesDecorationsClassName: `resource-studio-change-gutter-${change.kind}`,
          lineNumberClassName: `resource-studio-change-line-number-${change.kind}`,
          stickiness: monaco.editor.TrackedRangeStickiness.NeverGrowsWhenTypingAtEdges,
          overviewRuler: {
            color: colorByKind[change.kind],
            position: monaco.editor.OverviewRulerLane.Left,
          },
        },
      }),
    )
    const collection = editorInstance.createDecorationsCollection(decorations)
    return () => collection.clear()
  }, [dirty, editorInstance, lineDiff.ranges, monaco, viewMode])

  const handleEditorMount = useCallback<OnMount>((editor) => {
    editorRef.current = editor
    setEditorInstance(editor)
    editor.onDidChangeCursorPosition((event) => {
      setCursor({ line: event.position.lineNumber, column: event.position.column })
    })
  }, [])

  const beginResize = (event: React.PointerEvent<HTMLDivElement>) => {
    if (!rootRef.current || !isDesktop) return
    event.preventDefault()
    const root = rootRef.current
    const move = (pointerEvent: PointerEvent) => {
      const bounds = root.getBoundingClientRect()
      const maxByEditor = bounds.width - RESOURCE_STUDIO_TREE.MIN_EDITOR_WIDTH
      const next = clamp(
        bounds.right - pointerEvent.clientX,
        RESOURCE_STUDIO_TREE.MIN_WIDTH,
        Math.min(RESOURCE_STUDIO_TREE.MAX_WIDTH, maxByEditor),
      )
      setTreeWidth(next)
      localStorage.setItem(RESOURCE_STUDIO_TREE.STORAGE_KEY, String(next))
    }
    const stop = () => {
      window.removeEventListener("pointermove", move)
      window.removeEventListener("pointerup", stop)
    }
    window.addEventListener("pointermove", move)
    window.addEventListener("pointerup", stop)
  }

  const toggleDirectory = (path: string) => {
    setExpandedDirectories((previous) => {
      const next = new Set(previous)
      if (next.has(path)) next.delete(path)
      else next.add(path)
      return next
    })
  }

  const selectTreeEntry = (path: string, kind: ResourceStudioEntryKind) => {
    setFocusedPath(path)
    setFocusedKind(kind)
    if (kind === RESOURCE_STUDIO_ENTRY_KIND.DIRECTORY) {
      toggleDirectory(path)
      return
    }
    onSelectFile(path)
  }

  const submitEntryEditor = async () => {
    if (!entryEditor) return
    const destination = normalizePath(entryEditor.path)
    if (!destination) return
    try {
      if (entryEditor.action === "create") {
        await onCreateFile(destination)
      } else if (focusedPath) {
        await onMoveEntry(focusedPath, destination)
      }
      setEntryEditor(null)
    } catch {
      // The mutation error stays visible in the editor status bar for retry.
    }
  }

  const deleteFocused = async () => {
    if (!focusedPath || focusedIsProtected) return
    if (!window.confirm(`Delete ${focusedPath} from this Draft?`)) return
    try {
      await onDeleteEntry(focusedPath)
      setFocusedPath(selectedPath)
      setFocusedKind(RESOURCE_STUDIO_ENTRY_KIND.FILE)
    } catch {
      // The mutation error stays visible in the editor status bar for retry.
    }
  }

  const copyContent = async () => {
    await navigator.clipboard.writeText(
      viewMode === RESOURCE_STUDIO_VIEW_MODE.FILE ? savedContent : editorValue,
    )
    setCopied(true)
    window.setTimeout(() => setCopied(false), 1_500)
  }

  const downloadContent = () => {
    if (!selectedPath) return
    const link = document.createElement("a")
    link.href = URL.createObjectURL(new Blob([editorValue], { type: "text/plain;charset=utf-8" }))
    link.download = selectedPath.split("/").at(-1) ?? "resource-file.txt"
    link.click()
    URL.revokeObjectURL(link.href)
  }

  const jumpToDiagnostic = (diagnostic: ResourceDiagnostic) => {
    if (diagnostic.path) onSelectFile(diagnostic.path)
    const line = Math.max(1, diagnostic.line ?? 1)
    setPendingReveal({ path: diagnostic.path ?? selectedPath ?? "", line })
    setViewMode(RESOURCE_STUDIO_VIEW_MODE.EDIT)
    if (!isDesktop) setTreeVisible(false)
  }

  if (loading) return <SkeletonRows rows={10} />
  if (loadError) {
    return (
      <ErrorState
        message={loadError instanceof Error ? loadError.message : "Resource Studio failed"}
      />
    )
  }

  return (
    <div
      ref={rootRef}
      className={cn(
        "flex h-[calc(100dvh-13.5rem)] min-h-[520px] flex-col overflow-hidden border border-(--border-card) bg-(--bg-card) shadow-sm lg:min-h-[480px]",
        expanded
          ? "fixed inset-0 z-(--z-modal) h-dvh min-h-0 rounded-none"
          : "relative rounded-xl",
      )}
      style={
        !isDesktop && !expanded
          ? { height: "auto", minHeight: RESOURCE_STUDIO_LAYOUT.MOBILE_MIN_HEIGHT }
          : undefined
      }
      data-testid="resource-studio"
    >
      <WorkbenchHeader
        resource={resource}
        draft={draft}
        selectedPath={selectedPath}
        savedContent={savedContent}
        dirty={dirty}
        lineDiff={lineDiff}
        saving={saving}
        viewMode={viewMode}
        canPreview={canPreview}
        copied={copied}
        treeVisible={treeVisible}
        expanded={expanded}
        onViewMode={setViewMode}
        onCopy={() => void copyContent()}
        onDownload={downloadContent}
        onToggleTree={() => setTreeVisible((visible) => !visible)}
        onToggleExpanded={() => setExpanded((value) => !value)}
        onDiscard={onDiscard}
        onSave={onSave}
      />

      <div className="flex min-h-0 flex-1 flex-col overflow-hidden lg:flex-row">
        <main className="order-1 flex min-h-[360px] min-w-0 flex-1 flex-col bg-(--bg-card)">
          {!selectedFile ? (
            <EmptyState
              className="m-auto border-0 bg-transparent"
              icon={FilePlus2}
              title="Create the first draft file"
              description="Use New file in the Files panel. Nested paths create their parent folders automatically."
              action={
                <Button
                  onClick={() =>
                    setEntryEditor({
                      action: "create",
                      path: RESOURCE_STUDIO_DEFAULT_NEW_FILE,
                    })
                  }
                >
                  <FilePlus2 /> New file
                </Button>
              }
            />
          ) : viewMode === RESOURCE_STUDIO_VIEW_MODE.PREVIEW && canPreview ? (
            <ResourcePreview path={selectedPath ?? ""} content={editorValue} />
          ) : viewMode === RESOURCE_STUDIO_VIEW_MODE.DIFF ? (
            dirty ? (
              <Suspense fallback={<EditorLoading />}>
                <MonacoDiffEditor
                  height="100%"
                  language={resourceStudioLanguage(selectedPath)}
                  theme={monacoTheme}
                  original={savedContent}
                  modified={editorValue}
                  options={diffEditorOptions(isDesktop)}
                />
              </Suspense>
            ) : (
              <div className="grid h-full place-items-center text-xs text-(--color-text-subtle)">
                <span className="flex items-center gap-2"><GitCompare className="size-4" /> No unsaved changes to compare.</span>
              </div>
            )
          ) : (
            <Suspense fallback={<EditorLoading />}>
              <MonacoEditor
                height="100%"
                path={`conductor-resource://${resource.id}/${selectedPath}`}
                language={resourceStudioLanguage(selectedPath)}
                theme={monacoTheme}
                value={
                  viewMode === RESOURCE_STUDIO_VIEW_MODE.FILE ? savedContent : editorValue
                }
                onMount={handleEditorMount}
                onChange={
                  viewMode === RESOURCE_STUDIO_VIEW_MODE.EDIT
                    ? (value) => onChange(value ?? "")
                    : undefined
                }
                options={editorOptions(viewMode === RESOURCE_STUDIO_VIEW_MODE.EDIT)}
              />
            </Suspense>
          )}
          <EditorStatusBar
            dirty={dirty}
            lineDiff={lineDiff}
            validation={validation}
            actionError={actionErrorMessage}
            path={selectedPath}
            content={editorValue}
            cursor={cursor}
          />
        </main>

        {treeVisible && (
          <>
            <div
              role="separator"
              aria-label="Resize Resource Studio file tree"
              aria-orientation="vertical"
              aria-valuemin={RESOURCE_STUDIO_TREE.MIN_WIDTH}
              aria-valuemax={RESOURCE_STUDIO_TREE.MAX_WIDTH}
              aria-valuenow={Math.round(treeWidth)}
              onPointerDown={beginResize}
              onDoubleClick={() => {
                setTreeWidth(RESOURCE_STUDIO_TREE.DEFAULT_WIDTH)
                localStorage.setItem(
                  RESOURCE_STUDIO_TREE.STORAGE_KEY,
                  String(RESOURCE_STUDIO_TREE.DEFAULT_WIDTH),
                )
              }}
              title="Drag to resize · double-click to reset"
              className="group relative order-2 hidden w-2 shrink-0 cursor-ew-resize lg:block"
            >
              <span className="pointer-events-none absolute inset-y-0 left-1/2 w-px -translate-x-1/2 bg-(--color-border) transition-colors group-hover:bg-(--color-accent)/60" />
            </div>
            <aside
              className="order-3 flex min-h-56 w-full shrink-0 flex-col border-t border-(--color-border) bg-(--bg-page) lg:min-h-0 lg:border-t-0"
              style={isDesktop ? { width: treeWidth } : undefined}
              aria-label="Resource draft navigation"
            >
              <SidePanelTabs
                active={activePanel}
                diagnostics={diagnostics.length}
                onChange={setActivePanel}
              />
              {activePanel === RESOURCE_STUDIO_PANEL.FILES ? (
                <FilesPanel
                  tree={tree}
                  totalFiles={draft?.files.length ?? 0}
                  visibleFiles={visibleFiles.length}
                  query={query}
                  searchRef={searchRef}
                  focusedPath={focusedPath}
                  selectedPath={selectedPath}
                  dirtyPath={dirty ? selectedPath : null}
                  expandedDirectories={expandedDirectories}
                  entryEditor={entryEditor}
                  busy={busyAction}
                  draftLocked={dirty}
                  canRename={Boolean(focusedPath) && !focusedIsProtected && !dirty}
                  canDelete={Boolean(focusedPath) && !focusedIsProtected && !dirty}
                  onQuery={setQuery}
                  onSelectEntry={selectTreeEntry}
                  onNewFile={() =>
                    setEntryEditor({
                      action: "create",
                      path: focusedKind === RESOURCE_STUDIO_ENTRY_KIND.DIRECTORY && focusedPath
                        ? `${focusedPath}/new-file.md`
                        : RESOURCE_STUDIO_DEFAULT_NEW_FILE,
                    })
                  }
                  onRename={() =>
                    focusedPath && setEntryEditor({ action: "rename", path: focusedPath })
                  }
                  onDelete={() => void deleteFocused()}
                  onRefresh={onRefresh}
                  onEntryEditor={setEntryEditor}
                  onSubmitEntry={() => void submitEntryEditor()}
                />
              ) : activePanel === RESOURCE_STUDIO_PANEL.PROBLEMS ? (
                <ProblemsPanel
                  validation={validation}
                  errorCount={errorCount}
                  warningCount={warningCount}
                  onSelect={jumpToDiagnostic}
                />
              ) : (
                <GuidePanel resource={resource} requiredEntry={requiredEntry} />
              )}
            </aside>
          </>
        )}
      </div>
    </div>
  )
}

function WorkbenchHeader({
  resource,
  draft,
  selectedPath,
  savedContent,
  dirty,
  lineDiff,
  saving,
  viewMode,
  canPreview,
  copied,
  treeVisible,
  expanded,
  onViewMode,
  onCopy,
  onDownload,
  onToggleTree,
  onToggleExpanded,
  onDiscard,
  onSave,
}: {
  resource: ManagedResource
  draft?: DraftFileTree
  selectedPath: string | null
  savedContent: string
  dirty: boolean
  lineDiff: ResourceLineDiff
  saving: boolean
  viewMode: ResourceStudioViewMode
  canPreview: boolean
  copied: boolean
  treeVisible: boolean
  expanded: boolean
  onViewMode: (mode: ResourceStudioViewMode) => void
  onCopy: () => void
  onDownload: () => void
  onToggleTree: () => void
  onToggleExpanded: () => void
  onDiscard: () => void
  onSave: () => void
}) {
  const modes = [
    { value: RESOURCE_STUDIO_VIEW_MODE.FILE, label: "File", icon: FileText, visible: true },
    { value: RESOURCE_STUDIO_VIEW_MODE.PREVIEW, label: "Preview", icon: Eye, visible: canPreview },
    { value: RESOURCE_STUDIO_VIEW_MODE.EDIT, label: "Edit", icon: Pencil, visible: true },
    { value: RESOURCE_STUDIO_VIEW_MODE.DIFF, label: "Diff", icon: GitCompare, visible: true },
  ]
  return (
    <header className="shrink-0 border-b border-(--border-soft)">
      <div className="flex min-h-12 flex-wrap items-center justify-between gap-2 px-3 py-2">
        <div className="min-w-0 flex-1">
          <div className="flex min-w-0 items-center gap-2">
            <FileTypeIcon name={selectedPath ?? ""} size={16} />
            <span className="truncate font-mono text-xs font-medium" title={selectedPath ?? ""}>
              {selectedPath ?? "No file selected"}
            </span>
            {dirty && <span className="size-2 shrink-0 rounded-full bg-(--color-change)" title="Unsaved changes" />}
          </div>
          <div className="mt-0.5 flex flex-wrap items-center gap-x-2 text-[0.65rem] text-(--color-text-subtle)">
            <span>{formatBytes(contentBytes(savedContent))}</span>
            <span>{resourceStudioLanguage(selectedPath)}</span>
            <span>Draft #{draft?.revision ?? resource.draft_revision}</span>
            {dirty && (
              <span className="inline-flex items-center gap-1 text-(--color-change)">
                <GitCompare className="size-3" />
                {lineDiff.changedLines} changed
                <ChangeStats diff={lineDiff} />
              </span>
            )}
          </div>
        </div>

        <div className="order-3 flex w-full items-center gap-1 overflow-x-auto sm:order-2 sm:w-auto">
          <div className="flex rounded-md border border-(--color-border) bg-(--bg-page) p-0.5">
            {modes.filter((mode) => mode.visible).map((mode) => {
              const Icon = mode.icon
              return (
                <button
                  key={mode.value}
                  type="button"
                  onClick={() => onViewMode(mode.value)}
                  aria-pressed={viewMode === mode.value}
                  className={cn(
                    "flex h-7 items-center gap-1 rounded-sm px-2 text-xs transition-colors",
                    viewMode === mode.value
                      ? "bg-(--bg-key) text-(--color-text) shadow-sm"
                      : "text-(--color-text-muted) hover:text-(--color-text)",
                  )}
                >
                  <Icon className="size-3" /> {mode.label}
                </button>
              )
            })}
          </div>
        </div>

        <div className="order-2 flex shrink-0 items-center gap-0.5 sm:order-3">
          {dirty && (
            <Button variant="ghost" size="sm" onClick={onDiscard} disabled={saving}>
              <Undo2 /> <span className="hidden xl:inline">Discard</span>
            </Button>
          )}
          <Button size="sm" onClick={onSave} disabled={!dirty || saving}>
            {saving ? <Loader2 className="animate-spin" /> : <Save />}
            <span className="hidden xl:inline">{saving ? "Saving…" : "Save"}</span>
          </Button>
          <IconButton label={copied ? "Copied" : "Copy file"} onClick={onCopy} disabled={!selectedPath}>
            {copied ? <Check className="text-(--color-success)" /> : <Copy />}
          </IconButton>
          <IconButton label="Download file" onClick={onDownload} disabled={!selectedPath}>
            <Download />
          </IconButton>
          <IconButton
            label={treeVisible ? "Hide file tree" : "Show file tree"}
            onClick={onToggleTree}
            pressed={treeVisible}
          >
            {treeVisible ? <PanelRightClose /> : <PanelRightOpen />}
          </IconButton>
          <IconButton
            label={expanded ? "Exit expanded editor" : "Expand editor"}
            onClick={onToggleExpanded}
            pressed={expanded}
          >
            {expanded ? <Minimize2 /> : <Maximize2 />}
          </IconButton>
        </div>
      </div>
      <div className="flex min-h-8 items-center gap-2 overflow-x-auto border-t border-(--border-soft) bg-(--bg-page)/45 px-3 text-[0.68rem] text-(--color-text-muted)">
        <Badge>{RESOURCE_KIND_LABEL[resource.kind]}</Badge>
        <span className="capitalize">{resource.status}</span>
        <span aria-hidden="true">·</span>
        <span>{resource.release_channel ?? "Unreleased"}</span>
        <span aria-hidden="true">·</span>
        <span>{resource.highest_version ? `v${resource.highest_version}` : "First release v0.1.0"}</span>
      </div>
    </header>
  )
}

function SidePanelTabs({
  active,
  diagnostics,
  onChange,
}: {
  active: ResourceStudioPanel
  diagnostics: number
  onChange: (panel: ResourceStudioPanel) => void
}) {
  const tabs = [
    { value: RESOURCE_STUDIO_PANEL.FILES, label: "Files" },
    { value: RESOURCE_STUDIO_PANEL.PROBLEMS, label: "Problems", count: diagnostics },
    { value: RESOURCE_STUDIO_PANEL.GUIDE, label: "Guide" },
  ]
  return (
    <div className="flex shrink-0 border-b border-(--border-soft) px-1">
      {tabs.map((tab) => (
        <button
          key={tab.value}
          type="button"
          onClick={() => onChange(tab.value)}
          className={cn(
            "flex min-h-9 flex-1 items-center justify-center gap-1 border-b-2 px-1 text-[0.68rem] font-medium",
            active === tab.value
              ? "border-(--color-accent) text-(--color-text)"
              : "border-transparent text-(--color-text-subtle) hover:text-(--color-text)",
          )}
        >
          {tab.label}
          {tab.count ? (
            <span className="rounded-full bg-(--color-error-subtle) px-1.5 text-[0.62rem] text-(--color-error)">
              {tab.count}
            </span>
          ) : null}
        </button>
      ))}
    </div>
  )
}

function FilesPanel({
  tree,
  totalFiles,
  visibleFiles,
  query,
  searchRef,
  focusedPath,
  selectedPath,
  dirtyPath,
  expandedDirectories,
  entryEditor,
  busy,
  draftLocked,
  canRename,
  canDelete,
  onQuery,
  onSelectEntry,
  onNewFile,
  onRename,
  onDelete,
  onRefresh,
  onEntryEditor,
  onSubmitEntry,
}: {
  tree: TreeNode
  totalFiles: number
  visibleFiles: number
  query: string
  searchRef: React.RefObject<HTMLInputElement | null>
  focusedPath: string | null
  selectedPath: string | null
  dirtyPath: string | null
  expandedDirectories: Set<string>
  entryEditor: EntryEditor | null
  busy: boolean
  draftLocked: boolean
  canRename: boolean
  canDelete: boolean
  onQuery: (value: string) => void
  onSelectEntry: (path: string, kind: ResourceStudioEntryKind) => void
  onNewFile: () => void
  onRename: () => void
  onDelete: () => void
  onRefresh: () => void
  onEntryEditor: (value: EntryEditor | null) => void
  onSubmitEntry: () => void
}) {
  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="shrink-0 border-b border-(--color-border) px-2 py-1.5">
        <div className="flex items-center gap-1.5 rounded-md border border-(--color-border) bg-(--bg-card) px-2 py-1">
          <Search size={12} className="shrink-0 text-(--color-text-subtle)" />
          <input
            ref={searchRef}
            type="text"
            value={query}
            onChange={(event) => onQuery(event.target.value)}
            placeholder="Search files…"
            className="w-full bg-transparent text-xs text-(--color-text) outline-none placeholder:text-(--color-text-subtle)"
            aria-label="Search draft files"
          />
          {query && (
            <button
              type="button"
              onClick={() => {
                onQuery("")
                searchRef.current?.focus()
              }}
              className="shrink-0 rounded p-0.5 text-(--color-text-subtle) hover:text-(--color-text)"
              aria-label="Clear file search"
            >
              <X size={10} />
            </button>
          )}
        </div>
      </div>
      <div className="flex shrink-0 items-center justify-between gap-2 border-b border-(--color-border) px-2 py-1">
        <span className="text-[0.65rem] text-(--color-text-subtle)">
          {query ? `${visibleFiles} of ${totalFiles}` : `${totalFiles} files`}
        </span>
        <div className="flex items-center gap-0.5">
          <IconButton label={draftLocked ? "Save changes before creating a file" : "New file"} onClick={onNewFile} disabled={busy || draftLocked}>
            <FilePlus2 />
          </IconButton>
          <IconButton label="Rename selected entry" onClick={onRename} disabled={!canRename || busy}>
            <Pencil />
          </IconButton>
          <IconButton label="Delete selected entry" onClick={onDelete} disabled={!canDelete || busy}>
            <Trash2 />
          </IconButton>
          <IconButton label="Refresh draft files" onClick={onRefresh} disabled={busy}>
            <RefreshCw className={cn(busy && "animate-spin")} />
          </IconButton>
        </div>
      </div>
      {entryEditor && (
        <div className="shrink-0 space-y-2 border-b border-(--border-soft) bg-(--bg-page)/55 p-2">
          <label className="block text-[0.65rem] font-medium text-(--color-text-muted)">
            {entryEditor.action === "create" ? "New file path" : "New path"}
          </label>
          <Input
            autoFocus
            value={entryEditor.path}
            onChange={(event) => onEntryEditor({ ...entryEditor, path: event.target.value })}
            onKeyDown={(event) => {
              if (event.key === "Enter") onSubmitEntry()
              if (event.key === "Escape") onEntryEditor(null)
            }}
            className="font-mono text-xs"
            aria-label={entryEditor.action === "create" ? "New draft file path" : "Renamed draft path"}
          />
          <div className="flex justify-end gap-1">
            <Button variant="ghost" size="sm" onClick={() => onEntryEditor(null)}>
              Cancel
            </Button>
            <Button size="sm" disabled={!normalizePath(entryEditor.path) || busy} onClick={onSubmitEntry}>
              {busy && <Loader2 className="animate-spin" />}
              {entryEditor.action === "create" ? "Create" : "Rename"}
            </Button>
          </div>
        </div>
      )}
      <div className="min-h-0 flex-1 overflow-auto p-2">
        {visibleFiles === 0 ? (
          <p className="px-2 py-4 text-xs italic text-(--color-text-subtle)">
            {query ? `No files match "${query}"` : "No files shown"}
          </p>
        ) : (
          <TreeNodeView
            node={tree}
            depth={0}
            focusedPath={focusedPath}
            selectedPath={selectedPath}
            dirtyPath={dirtyPath}
            forceOpen={Boolean(query.trim())}
            expandedDirectories={expandedDirectories}
            onSelectEntry={onSelectEntry}
          />
        )}
      </div>
    </div>
  )
}

function TreeNodeView({
  node,
  depth,
  focusedPath,
  selectedPath,
  dirtyPath,
  forceOpen = false,
  expandedDirectories,
  onSelectEntry,
}: {
  node: TreeNode
  depth: number
  focusedPath: string | null
  selectedPath: string | null
  dirtyPath: string | null
  forceOpen?: boolean
  expandedDirectories: Set<string>
  onSelectEntry: (path: string, kind: ResourceStudioEntryKind) => void
}) {
  const children = [...node.children.values()].sort((left, right) => {
    const leftDirectory = !left.file
    const rightDirectory = !right.file
    if (leftDirectory !== rightDirectory) return leftDirectory ? -1 : 1
    return RESOURCE_TREE_NAME_COLLATOR.compare(left.name, right.name)
  })
  return children.map((child) => {
    const kind = child.file
      ? RESOURCE_STUDIO_ENTRY_KIND.FILE
      : RESOURCE_STUDIO_ENTRY_KIND.DIRECTORY
    const isDirectory = kind === RESOURCE_STUDIO_ENTRY_KIND.DIRECTORY
    const isExpanded = forceOpen || expandedDirectories.has(child.path)
    const isFocused = focusedPath === child.path
    const isSelectedFile = selectedPath === child.path
    const isDirty = dirtyPath === child.path
    const hasDirtyDescendant = isDirectory && pathHasDescendant(child.path, dirtyPath)
    return (
      <div key={child.path}>
        <button
          type="button"
          onClick={() => onSelectEntry(child.path, kind)}
          className={cn(
            "flex w-full items-center gap-1.5 rounded px-2 py-1 text-left text-xs transition-colors",
            isFocused || isSelectedFile
              ? "bg-(--bg-key) text-(--color-accent)"
              : hasDirtyDescendant
                ? "text-(--color-text) hover:bg-(--bg-key)"
                : "text-(--color-text-2) hover:bg-(--bg-key) hover:text-(--color-text)",
          )}
          style={{ paddingLeft: 8 + depth * 12 }}
          title={child.path}
          aria-expanded={isDirectory ? isExpanded : undefined}
        >
          {isDirectory ? (
            <>
              <ChevronRight
                size={12}
                className={cn("shrink-0 transition-transform", isExpanded && "rotate-90")}
              />
              <FolderTypeIcon open={isExpanded} size={16} />
            </>
          ) : (
            <>
              <span className="w-3 shrink-0" />
              <FileTypeIcon name={child.name} size={16} />
            </>
          )}
          <span className="min-w-0 flex-1 truncate font-mono">{child.name}</span>
          {hasDirtyDescendant && (
            <span
              className="size-1.5 shrink-0 rounded-full bg-(--color-change)"
              aria-label="Contains modified files"
            />
          )}
          {child.file && (
            <>
              {isDirty && (
                <span
                  className="shrink-0 font-mono text-xs font-semibold text-(--color-change)"
                  title="Modified locally"
                  aria-label="Modified locally"
                >
                  M
                </span>
              )}
              <span className="shrink-0 text-xs text-(--color-text-subtle)">
                {formatBytes(contentBytes(child.file.content))}
              </span>
            </>
          )}
        </button>
        {isDirectory && isExpanded && (
          <TreeNodeView
            node={child}
            depth={depth + 1}
            focusedPath={focusedPath}
            selectedPath={selectedPath}
            dirtyPath={dirtyPath}
            forceOpen={forceOpen}
            expandedDirectories={expandedDirectories}
            onSelectEntry={onSelectEntry}
          />
        )}
      </div>
    )
  })
}

function ProblemsPanel({
  validation,
  errorCount,
  warningCount,
  onSelect,
}: {
  validation: ResourceValidation | null
  errorCount: number
  warningCount: number
  onSelect: (diagnostic: ResourceDiagnostic) => void
}) {
  if (!validation) {
    return (
      <div className="p-3">
        <div className="rounded-lg border border-dashed border-(--color-border) p-3 text-xs leading-5 text-(--color-text-subtle)">
          Save your edits and run Validate. Problems will link directly to their source file and line.
        </div>
      </div>
    )
  }
  if (!validation.diagnostics.length) {
    return (
      <div className="flex items-start gap-2 p-4 text-xs text-(--color-success)">
        <CheckCircle2 className="mt-0.5 size-4 shrink-0" />
        Draft revision {validation.revision} passes static validation.
      </div>
    )
  }
  return (
    <div className="min-h-0 flex-1 overflow-auto">
      <div className="sticky top-0 z-1 flex gap-3 border-b border-(--border-soft) bg-(--bg-card) px-3 py-2 text-[0.68rem]">
        <span className="text-(--color-error)">{errorCount} errors</span>
        <span className="text-(--color-warning)">{warningCount} warnings</span>
      </div>
      <div className="divide-y divide-(--border-soft)" data-testid="resource-diagnostics">
        {validation.diagnostics.map((item, index) => (
          <button
            key={`${item.code}-${item.path}-${index}`}
            type="button"
            onClick={() => onSelect(item)}
            className="flex w-full items-start gap-2 p-3 text-left transition-colors hover:bg-(--bg-key)/60"
          >
            <AlertTriangle
              className={cn(
                "mt-0.5 size-3.5 shrink-0",
                item.severity === "error" ? "text-(--color-error)" : "text-(--color-warning)",
              )}
            />
            <span className="min-w-0">
              <span className="block text-xs font-medium">{item.code}</span>
              <span className="mt-1 block text-[0.68rem] leading-4 text-(--color-text-muted)">
                {item.message}
              </span>
              {item.path && (
                <span className="mt-1 block truncate font-mono text-[0.62rem] text-(--color-accent)">
                  {item.path}{item.line ? `:${item.line}` : ""}
                </span>
              )}
            </span>
          </button>
        ))}
      </div>
    </div>
  )
}

function GuidePanel({
  resource,
  requiredEntry,
}: {
  resource: ManagedResource
  requiredEntry: string
}) {
  const description =
    resource.kind === RESOURCE_KIND.PLUGIN
      ? "Keep plugin.json at the package root. Published archives arrive disabled in EvoFlux until the member reviews commands, hosts, and capabilities."
      : resource.kind === RESOURCE_KIND.SKILL
        ? "SKILL.md needs name and description frontmatter. Additional references and scripts can live in nested folders."
        : "Agent Markdown needs matching name and description frontmatter. Validation is static and never executes draft content."
  return (
    <div className="space-y-4 overflow-auto p-3">
      <div>
        <div className="flex items-center gap-2 text-xs font-medium">
          <Sparkles className="size-3.5 text-(--color-accent)" />
          {RESOURCE_KIND_LABEL[resource.kind]} authoring guide
        </div>
        <p className="mt-2 text-xs leading-5 text-(--color-text-muted)">{description}</p>
      </div>
      <div className="rounded-lg border border-(--border-soft) bg-(--bg-page)/45 p-3">
        <div className="text-[0.65rem] font-semibold tracking-wide text-(--color-text-subtle) uppercase">
          Required entry
        </div>
        <code className="mt-2 block font-mono text-xs text-(--color-accent)">{requiredEntry}</code>
      </div>
      <div className="flex gap-2 rounded-lg border border-(--border-soft) p-3 text-xs leading-5 text-(--color-text-muted)">
        <Info className="mt-0.5 size-3.5 shrink-0 text-(--color-info)" />
        Saving creates a new Draft revision. Versions are allocated only when you release to Beta or Published.
      </div>
    </div>
  )
}

function EditorStatusBar({
  dirty,
  lineDiff,
  validation,
  actionError,
  path,
  content,
  cursor,
}: {
  dirty: boolean
  lineDiff: ResourceLineDiff
  validation: ResourceValidation | null
  actionError: string | null
  path: string | null
  content: string
  cursor: { line: number; column: number }
}) {
  return (
    <div
      className="flex min-h-7 shrink-0 items-center justify-between gap-3 overflow-x-auto border-t border-(--border-soft) bg-(--bg-page)/55 px-3 text-[0.65rem] text-(--color-text-subtle)"
      role="status"
      aria-live="polite"
    >
      <div className="flex min-w-0 items-center gap-2">
        {actionError ? (
          <span className="truncate text-(--color-error)" title={actionError}>{actionError}</span>
        ) : dirty ? (
          <span className="text-(--color-change)">
            Unsaved · {lineDiff.changedLines} changed lines
          </span>
        ) : validation?.valid ? (
          <span className="text-(--color-success)">Validation passed</span>
        ) : validation ? (
          <span className="text-(--color-error)">{validation.diagnostics.length} validation issues</span>
        ) : (
          <span>Static validation not run</span>
        )}
      </div>
      <div className="flex shrink-0 items-center gap-3 font-mono">
        {dirty && <ChangeStats diff={lineDiff} />}
        <span>Ln {cursor.line}, Col {cursor.column}</span>
        <span>{content.split("\n").length} lines</span>
        <span>{resourceStudioLanguage(path)}</span>
      </div>
    </div>
  )
}

function ChangeStats({ diff }: { diff: ResourceLineDiff }) {
  return (
    <span
      className="inline-flex items-center gap-1.5 font-mono font-semibold"
      aria-label={`${diff.addedLines} added, ${diff.modifiedLines} modified, ${diff.removedLines} removed lines`}
    >
      {diff.addedLines > 0 && (
        <span className="text-(--color-success)">+{diff.addedLines}</span>
      )}
      {diff.modifiedLines > 0 && (
        <span className="text-(--color-change)">~{diff.modifiedLines}</span>
      )}
      {diff.removedLines > 0 && (
        <span className="text-(--color-error)">-{diff.removedLines}</span>
      )}
    </span>
  )
}

function ResourcePreview({ path, content }: { path: string; content: string }) {
  const extension = resourceStudioExtension(path)
  if (extension === "html" || extension === "htm") {
    return (
      <iframe
        title={`${path} preview`}
        srcDoc={content}
        sandbox=""
        className="h-full min-h-0 w-full border-0 bg-white"
      />
    )
  }
  return <MarkdownPreview content={content} />
}

function MarkdownPreview({ content }: { content: string }) {
  const blocks = content.split(/\n{2,}/)
  return (
    <div className="h-full overflow-auto bg-(--bg-page) p-6">
      <article className="mx-auto max-w-3xl space-y-4 text-sm leading-7 text-(--color-text-2)">
        {blocks.map((block, index) => {
          const trimmed = block.trim()
          if (!trimmed) return null
          if (trimmed.startsWith("```")) {
            const lines = trimmed.split("\n")
            return (
              <pre key={index} className="overflow-x-auto rounded-lg border border-(--border-soft) bg-(--bg-card) p-4 font-mono text-xs leading-5">
                <code>{lines.slice(1, lines.at(-1)?.startsWith("```") ? -1 : undefined).join("\n")}</code>
              </pre>
            )
          }
          const heading = /^(#{1,4})\s+(.+)$/.exec(trimmed)
          if (heading) {
            const level = heading[1].length
            return (
              <div key={index} className={cn("font-semibold tracking-tight text-(--color-text)", level === 1 ? "text-2xl" : level === 2 ? "text-xl" : "text-base")}>
                {heading[2]}
              </div>
            )
          }
          const list = trimmed.split("\n").filter((line) => /^[-*]\s+/.test(line))
          if (list.length === trimmed.split("\n").length) {
            return (
              <ul key={index} className="list-disc space-y-1 pl-5">
                {list.map((line, itemIndex) => <li key={itemIndex}>{line.replace(/^[-*]\s+/, "")}</li>)}
              </ul>
            )
          }
          return <p key={index} className="whitespace-pre-wrap">{trimmed}</p>
        })}
      </article>
    </div>
  )
}

function IconButton({
  label,
  children,
  onClick,
  disabled,
  pressed,
}: {
  label: string
  children: React.ReactNode
  onClick: () => void
  disabled?: boolean
  pressed?: boolean
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled}
      title={label}
      aria-label={label}
      aria-pressed={pressed}
      className={cn(
        "grid size-7 shrink-0 place-items-center rounded-sm text-(--color-text-muted) transition-colors hover:bg-(--bg-key) hover:text-(--color-text) disabled:pointer-events-none disabled:opacity-35 [&_svg]:size-3.5",
        pressed && "bg-(--bg-key) text-(--color-text)",
      )}
    >
      {children}
    </button>
  )
}

function EditorLoading() {
  return (
    <div className="grid h-full place-items-center text-xs text-(--color-text-subtle)">
      <span className="flex items-center gap-2"><Loader2 className="size-4 animate-spin" /> Loading editor…</span>
    </div>
  )
}

function buildTree(files: DraftFile[]): TreeNode {
  const root: TreeNode = { name: "/", path: "", children: new Map() }
  for (const file of files) {
    const parts = file.path.split("/")
    let node = root
    parts.forEach((part, index) => {
      const path = parts.slice(0, index + 1).join("/")
      let child = node.children.get(part)
      if (!child) {
        child = { name: part, path, children: new Map() }
        node.children.set(part, child)
      }
      if (index === parts.length - 1) child.file = file
      node = child
    })
  }
  return root
}

function directoryPaths(files: DraftFile[]): string[] {
  return [...new Set(files.flatMap((file) => parentPaths(file.path)))]
}

function parentPaths(path: string | null): string[] {
  if (!path) return []
  const parts = path.split("/")
  return parts.slice(0, -1).map((_, index) => parts.slice(0, index + 1).join("/"))
}

function pathHasDescendant(path: string, descendantPath: string | null): boolean {
  return Boolean(descendantPath && descendantPath.startsWith(`${path}/`))
}

function normalizePath(path: string): string {
  return path.trim().replace(/^\/+|\/+$/g, "")
}

function clamp(value: number, minimum: number, maximum: number): number {
  return Math.min(Math.max(value, minimum), maximum)
}

function readTreeWidth(): number {
  const stored = Number(localStorage.getItem(RESOURCE_STUDIO_TREE.STORAGE_KEY))
  if (!Number.isFinite(stored)) return RESOURCE_STUDIO_TREE.DEFAULT_WIDTH
  return clamp(stored, RESOURCE_STUDIO_TREE.MIN_WIDTH, RESOURCE_STUDIO_TREE.MAX_WIDTH)
}

function editorOptions(editable: boolean) {
  return {
    readOnly: !editable,
    domReadOnly: !editable,
    ariaLabel: editable ? "Resource source editor" : "Resource source viewer",
    automaticLayout: true,
    minimap: { enabled: false },
    fontFamily: "JetBrains Mono Variable, monospace",
    fontSize: 13,
    lineHeight: 21,
    lineNumbers: "on" as const,
    lineDecorationsWidth: 10,
    folding: true,
    glyphMargin: false,
    renderLineHighlight: editable ? ("line" as const) : ("none" as const),
    scrollBeyondLastLine: false,
    wordWrap: "on" as const,
    padding: { top: 12, bottom: 12 },
    scrollbar: { verticalScrollbarSize: 8, horizontalScrollbarSize: 8, useShadows: false },
    overviewRulerLanes: editable ? 1 : 0,
    hideCursorInOverviewRuler: true,
    overviewRulerBorder: false,
  }
}

function diffEditorOptions(sideBySide: boolean) {
  return {
    readOnly: true,
    automaticLayout: true,
    renderSideBySide: sideBySide,
    minimap: { enabled: false },
    fontFamily: "JetBrains Mono Variable, monospace",
    fontSize: 12,
    lineHeight: 20,
    scrollBeyondLastLine: false,
    wordWrap: "on" as const,
    overviewRulerLanes: 0,
    padding: { top: 10, bottom: 10 },
  }
}

function formatBytes(value: number): string {
  if (value < 1_024) return `${value} B`
  if (value < 1_048_576) return `${(value / 1_024).toFixed(1)} KiB`
  return `${(value / 1_048_576).toFixed(1)} MiB`
}

function contentBytes(content: string): number {
  return new TextEncoder().encode(content).byteLength
}
