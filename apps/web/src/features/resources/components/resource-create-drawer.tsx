import { useMutation } from "@tanstack/react-query"
import {
  AlertTriangle,
  Bot,
  Boxes,
  CheckCircle2,
  FileArchive,
  FileCode2,
  PackagePlus,
  Sparkles,
  UploadCloud,
  Workflow,
} from "lucide-react"
import { useEffect, useRef, useState } from "react"

import {
  api,
  type ManagedResource,
  type ResourceArchiveInspection,
} from "@/shared/api/client"
import {
  RESOURCE_ARCHIVE_ACCEPT,
  RESOURCE_ARCHIVE_EXTENSIONS,
  RESOURCE_ARCHIVE_MAX_BYTES,
  RESOURCE_CREATE_COPY,
  RESOURCE_CREATE_MODE,
  RESOURCE_INITIAL_CHANGELOG,
  RESOURCE_INITIAL_VERSION,
  RESOURCE_KIND,
  RESOURCE_KIND_LABEL,
  RESOURCE_KIND_OPTIONS,
  RESOURCE_TARGET_MODES,
  RESOURCE_VISIBILITY_OPTIONS,
  type ResourceCreateMode,
  type ResourceKind,
  type ResourceTargetMode,
} from "@/shared/constants/resource"
import { ResourceModeSelector } from "@/features/resources/components/resource-mode-selector"
import { cn } from "@/shared/lib/utils"
import { Badge } from "@/shared/ui/badge"
import { Button } from "@/shared/ui/button"
import { Drawer } from "@/shared/ui/drawer"
import { ErrorState } from "@/shared/ui/empty-state"
import { Input } from "@/shared/ui/input"
import { Label } from "@/shared/ui/label"
import { Select } from "@/shared/ui/select"
import { Textarea } from "@/shared/ui/textarea"

const RESOURCE_CREATE_ICON = {
  [RESOURCE_KIND.AGENT]: Bot,
  [RESOURCE_KIND.SKILL]: Sparkles,
  [RESOURCE_KIND.PLUGIN]: PackagePlus,
  [RESOURCE_KIND.WORKFLOW]: Workflow,
  [RESOURCE_KIND.COMMAND]: FileCode2,
} as const

export function ResourceCreateDrawer({
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
  const fileInputRef = useRef<HTMLInputElement | null>(null)
  const selectedFileRef = useRef<File | null>(null)
  const [kind, setKind] = useState<ResourceKind>(defaultKind ?? RESOURCE_KIND.AGENT)
  const [mode, setMode] = useState<ResourceCreateMode>(RESOURCE_CREATE_MODE.UPLOAD)
  const [file, setFile] = useState<File | null>(null)
  const [inspection, setInspection] = useState<ResourceArchiveInspection | null>(null)
  const [inspectError, setInspectError] = useState<string | null>(null)
  const [localError, setLocalError] = useState<string | null>(null)
  const [dragActive, setDragActive] = useState(false)
  const [name, setName] = useState("")
  const [slug, setSlug] = useState("")
  const [slugEdited, setSlugEdited] = useState(false)
  const [description, setDescription] = useState("")
  const [visibility, setVisibility] = useState<ManagedResource["visibility"]>("shared")
  const [targetModes, setTargetModes] = useState<ResourceTargetMode[]>([
    ...RESOURCE_TARGET_MODES,
  ])
  const supportsArchive = kind === RESOURCE_KIND.AGENT || kind === RESOURCE_KIND.SKILL

  const inspect = useMutation({
    mutationFn: ({ archive, resourceKind }: { archive: File; resourceKind: ResourceKind }) =>
      api.inspectResourceArchive(resourceKind, archive),
    onSuccess: (result, variables) => {
      if (selectedFileRef.current !== variables.archive || kind !== variables.resourceKind) return
      setInspection(result)
      setInspectError(null)
      if (result.metadata.slug) {
        const inferredSlug = normalizeResourceSlug(variables.resourceKind, result.metadata.slug)
        setSlug(inferredSlug)
        setSlugEdited(true)
        setName((current) => current || titleFromSlug(result.metadata.slug!))
      }
      if (result.metadata.description) setDescription(result.metadata.description)
    },
    onError: (error, variables) => {
      if (selectedFileRef.current !== variables.archive) return
      setInspection(null)
      setInspectError(error instanceof Error ? error.message : "Package inspection failed")
    },
  })

  const create = useMutation({
    mutationFn: async () => {
      if (supportsArchive && mode === RESOURCE_CREATE_MODE.UPLOAD) {
        if (!file || !inspection) throw new Error("Choose and inspect a ZIP package first")
        const result = await api.createResourceFromArchive(kind, file, {
          slug,
          name,
          visibility,
          modes: targetModes,
        })
        return result.resource
      }
      return api.createResource({
        kind,
        slug,
        name,
        description,
        version: RESOURCE_INITIAL_VERSION,
        visibility,
        payload: supportsArchive ? { modes: targetModes } : {},
        changelog: RESOURCE_INITIAL_CHANGELOG,
      })
    },
    onSuccess: onCreated,
  })

  useEffect(() => {
    if (open) return
    selectedFileRef.current = null
    setKind(defaultKind ?? RESOURCE_KIND.AGENT)
    setMode(RESOURCE_CREATE_MODE.UPLOAD)
    setFile(null)
    setInspection(null)
    setInspectError(null)
    setLocalError(null)
    setDragActive(false)
    setName("")
    setSlug("")
    setSlugEdited(false)
    setDescription("")
    setVisibility("shared")
    setTargetModes([...RESOURCE_TARGET_MODES])
    inspect.reset()
    create.reset()
  }, [defaultKind, open])

  const copy = RESOURCE_CREATE_COPY[kind]
  const Icon = RESOURCE_CREATE_ICON[kind]
  const dirty =
    file !== null ||
    name.trim() !== "" ||
    slug.trim() !== "" ||
    description.trim() !== "" ||
    visibility !== "shared" ||
    targetModes.length !== RESOURCE_TARGET_MODES.length
  const isUpload = supportsArchive && mode === RESOURCE_CREATE_MODE.UPLOAD
  const createDisabled =
    create.isPending ||
    inspect.isPending ||
    !name.trim() ||
    !slug.trim() ||
    (isUpload && (!file || !inspection))

  function requestClose() {
    if (create.isPending || inspect.isPending) return
    if (dirty && !window.confirm(copy.discardMessage)) return
    onClose()
  }

  function resetArchive() {
    selectedFileRef.current = null
    setFile(null)
    setInspection(null)
    setInspectError(null)
    setLocalError(null)
    setDragActive(false)
    inspect.reset()
  }

  function changeKind(nextKind: ResourceKind) {
    resetArchive()
    setKind(nextKind)
    setMode(
      nextKind === RESOURCE_KIND.AGENT || nextKind === RESOURCE_KIND.SKILL
        ? RESOURCE_CREATE_MODE.UPLOAD
        : RESOURCE_CREATE_MODE.TEMPLATE,
    )
    setName("")
    setSlug("")
    setSlugEdited(false)
    setDescription("")
    setTargetModes([...RESOURCE_TARGET_MODES])
  }

  function chooseFile(nextFile: File | undefined) {
    if (!nextFile) return
    resetArchive()
    if (!hasAllowedExtension(nextFile.name)) {
      setLocalError("Choose a .zip package.")
      return
    }
    if (nextFile.size === 0 || nextFile.size > RESOURCE_ARCHIVE_MAX_BYTES) {
      setLocalError("Resource ZIP packages must be between 1 byte and 20 MiB.")
      return
    }
    selectedFileRef.current = nextFile
    setFile(nextFile)
    setName("")
    setSlug("")
    setSlugEdited(false)
    setDescription("")
    inspect.mutate({ archive: nextFile, resourceKind: kind })
  }

  return (
    <Drawer
      open={open}
      title={copy.title}
      description={copy.description}
      onClose={requestClose}
      footer={
        <>
          <Button
            variant="ghost"
            onClick={requestClose}
            disabled={create.isPending || inspect.isPending}
          >
            Cancel
          </Button>
          <Button
            variant="gradient"
            disabled={createDisabled}
            onClick={() => {
              setLocalError(null)
              create.mutate()
            }}
          >
            <Icon className="size-3.5" />
            {create.isPending ? "Creating…" : copy.createLabel}
          </Button>
        </>
      }
    >
      <div className="space-y-5">
        {defaultKind === undefined && (
          <Field label="Resource type" htmlFor="resource-kind">
            <Select
              id="resource-kind"
              value={kind}
              onValueChange={changeKind}
              options={RESOURCE_KIND_OPTIONS}
            />
          </Field>
        )}

        {supportsArchive && <CreateModeSelector value={mode} onChange={setMode} kind={kind} />}

        {(localError || inspectError || create.error) && (
          <ErrorState
            message={
              localError ??
              inspectError ??
              (create.error instanceof Error ? create.error.message : "Resource creation failed")
            }
          />
        )}

        {isUpload ? (
          <>
            <input
              ref={fileInputRef}
              type="file"
              className="sr-only"
              accept={RESOURCE_ARCHIVE_ACCEPT}
              onChange={(event) => {
                chooseFile(event.target.files?.[0])
                event.target.value = ""
              }}
            />
            <ArchiveDropzone
              kind={kind}
              active={dragActive}
              pending={inspect.isPending}
              onBrowse={() => fileInputRef.current?.click()}
              onDragActiveChange={setDragActive}
              onFile={chooseFile}
            />
            {file && (
              <ArchiveFileStatus file={file} pending={inspect.isPending} inspection={inspection} />
            )}
            {inspection && <ArchiveInspection inspection={inspection} />}
          </>
        ) : (
          <TemplateSummary kind={kind} />
        )}

        {(!isUpload || inspection) && (
          <div className="space-y-4 border-t border-(--border-soft) pt-5">
            <div className="grid gap-4 sm:grid-cols-2">
              <Field label="Display name" htmlFor="resource-name">
                <Input
                  id="resource-name"
                  value={name}
                  maxLength={120}
                  autoFocus={!isUpload}
                  onChange={(event) => {
                    const nextName = event.target.value
                    setName(nextName)
                    if (!slugEdited) setSlug(normalizeResourceSlug(kind, nextName))
                  }}
                />
              </Field>
              <Field
                label={`${RESOURCE_KIND_LABEL[kind]} slug`}
                htmlFor="resource-slug"
                hint={
                  kind === RESOURCE_KIND.AGENT
                    ? "Must match frontmatter name"
                    : "Lowercase letters, numbers and hyphens"
                }
              >
                <Input
                  id="resource-slug"
                  value={slug}
                  maxLength={80}
                  onChange={(event) => {
                    setSlugEdited(true)
                    setSlug(normalizeResourceSlug(kind, event.target.value))
                  }}
                />
              </Field>
            </div>

            {!isUpload && (
              <Field label="Description" htmlFor="resource-description">
                <Textarea
                  id="resource-description"
                  value={description}
                  maxLength={1000}
                  onChange={(event) => setDescription(event.target.value)}
                />
              </Field>
            )}

            <Field label="Default visibility" htmlFor="resource-visibility">
              <Select
                id="resource-visibility"
                value={visibility}
                onValueChange={setVisibility}
                options={RESOURCE_VISIBILITY_OPTIONS}
              />
            </Field>

            {supportsArchive && (
              <Field
                label="Available in EvoFlux"
                htmlFor="resource-target-modes"
                hint="Select at least one mode. Work is the cowork surface; Coding is repository-scoped."
              >
                <ResourceModeSelector value={targetModes} onChange={setTargetModes} />
              </Field>
            )}

            <div className="flex gap-2 rounded-lg border border-(--border-soft) bg-(--bg-page)/55 px-3 py-2.5 text-xs leading-relaxed text-(--color-text-muted)">
              <Boxes className="mt-0.5 size-3.5 shrink-0 text-(--color-accent)" />
              <span>
                This creates a mutable Draft only. Safe ZIPs with content errors can still be
                imported and repaired; validation errors block Beta and Published releases.
              </span>
            </div>
          </div>
        )}
      </div>
    </Drawer>
  )
}

function CreateModeSelector({
  value,
  onChange,
  kind,
}: {
  value: ResourceCreateMode
  onChange: (value: ResourceCreateMode) => void
  kind: ResourceKind
}) {
  const label = RESOURCE_KIND_LABEL[kind]
  const options = [
    {
      value: RESOURCE_CREATE_MODE.UPLOAD,
      icon: UploadCloud,
      title: "Upload ZIP",
      description: `Import an existing EvoFlux ${label}`,
    },
    {
      value: RESOURCE_CREATE_MODE.TEMPLATE,
      icon: FileCode2,
      title: "Blank template",
      description: "Build in Resource Studio",
    },
  ] as const

  return (
    <div className="grid grid-cols-2 gap-2" aria-label={`${label} creation method`}>
      {options.map((option) => {
        const ModeIcon = option.icon
        const active = value === option.value
        return (
          <button
            key={option.value}
            type="button"
            aria-pressed={active}
            onClick={() => onChange(option.value)}
            className={cn(
              "rounded-lg border px-3 py-3 text-left transition-colors outline-none focus-visible:ring-2 focus-visible:ring-(--focus-ring)/40",
              active
                ? "border-(--color-accent) bg-(--color-accent-soft)/45"
                : "border-(--border-soft) bg-(--bg-page)/45 hover:border-(--color-border-strong)",
            )}
          >
            <ModeIcon
              className={cn(
                "size-4",
                active ? "text-(--color-accent)" : "text-(--color-text-subtle)",
              )}
            />
            <span className="mt-2 block text-sm font-medium">{option.title}</span>
            <span className="mt-0.5 block text-[0.7rem] text-(--color-text-muted)">
              {option.description}
            </span>
          </button>
        )
      })}
    </div>
  )
}

function ArchiveDropzone({
  kind,
  active,
  pending,
  onBrowse,
  onDragActiveChange,
  onFile,
}: {
  kind: ResourceKind
  active: boolean
  pending: boolean
  onBrowse: () => void
  onDragActiveChange: (active: boolean) => void
  onFile: (file: File | undefined) => void
}) {
  const contract =
    kind === RESOURCE_KIND.AGENT
      ? "Exactly one root .md Agent definition"
      : "Root SKILL.md plus optional bundle files"
  const article = kind === RESOURCE_KIND.AGENT ? "an" : "a"
  return (
    <div
      className={cn(
        "rounded-xl border border-dashed p-6 text-center transition-colors",
        active
          ? "border-(--color-accent) bg-(--color-accent-soft)/50"
          : "border-(--color-border) bg-(--bg-page)/45",
      )}
      onDragEnter={(event) => {
        event.preventDefault()
        onDragActiveChange(true)
      }}
      onDragOver={(event) => event.preventDefault()}
      onDragLeave={(event) => {
        if (!event.currentTarget.contains(event.relatedTarget as Node | null)) {
          onDragActiveChange(false)
        }
      }}
      onDrop={(event) => {
        event.preventDefault()
        onDragActiveChange(false)
        onFile(event.dataTransfer.files[0])
      }}
    >
      <span className="mx-auto grid size-11 place-items-center rounded-xl border border-(--border-soft) bg-(--bg-key) text-(--color-accent)">
        <UploadCloud className="size-5" />
      </span>
      <p className="mt-3 text-sm font-medium">
        Drop {article} {RESOURCE_KIND_LABEL[kind]} ZIP here
      </p>
      <p className="mt-1 text-xs text-(--color-text-muted)">{contract}</p>
      <p className="mt-1 text-[0.7rem] text-(--color-text-subtle)">
        Editable UTF-8 files · one optional wrapper folder · up to 20 MiB
      </p>
      <Button
        type="button"
        variant="outline"
        size="sm"
        className="mt-4"
        onClick={onBrowse}
        disabled={pending}
      >
        <FileArchive className="size-3.5" />
        Browse ZIP
      </Button>
    </div>
  )
}

function ArchiveFileStatus({
  file,
  pending,
  inspection,
}: {
  file: File
  pending: boolean
  inspection: ResourceArchiveInspection | null
}) {
  return (
    <div className="flex items-center gap-3 rounded-lg border border-(--border-soft) bg-(--bg-key)/60 px-3 py-2.5">
      <FileArchive className="size-4 shrink-0 text-(--color-accent)" />
      <div className="min-w-0 flex-1">
        <p className="truncate text-sm font-medium">{file.name}</p>
        <p className="text-xs text-(--color-text-subtle)">{formatBytes(file.size)}</p>
      </div>
      {pending ? (
        <Badge>Inspecting…</Badge>
      ) : inspection?.validation.valid ? (
        <Badge className="border-(--color-success)/30 text-(--color-success)">Valid</Badge>
      ) : (
        <Badge className="border-(--color-warning)/30 text-(--color-warning)">
          Import with fixes
        </Badge>
      )}
    </div>
  )
}

function ArchiveInspection({ inspection }: { inspection: ResourceArchiveInspection }) {
  return (
    <div className="rounded-xl border border-(--border-soft) p-4">
      <div className="flex items-center justify-between gap-3">
        <div>
          <p className="text-sm font-medium">Package inspection</p>
          <p className="mt-0.5 text-xs text-(--color-text-muted)">
            Parsed as an EvoFlux {RESOURCE_KIND_LABEL[inspection.kind]} without executing content.
          </p>
        </div>
        {inspection.validation.valid ? (
          <CheckCircle2 className="size-5 text-(--color-success)" aria-label="Package is valid" />
        ) : (
          <AlertTriangle className="size-5 text-(--color-warning)" aria-label="Package needs fixes" />
        )}
      </div>

      <dl className="mt-4 grid grid-cols-2 gap-2">
        <SummaryItem label="Source" value={inspection.metadata.primary_source ?? "Missing"} mono />
        <SummaryItem label="Files" value={String(inspection.file_count)} />
        <SummaryItem label="EvoFlux name" value={inspection.metadata.slug ?? "Missing"} mono />
        <SummaryItem label="Extracted size" value={formatBytes(inspection.total_uncompressed_bytes)} />
      </dl>

      {inspection.metadata.description && (
        <p className="mt-3 rounded-lg bg-(--bg-page)/55 px-3 py-2 text-xs leading-relaxed text-(--color-text-muted)">
          {inspection.metadata.description}
        </p>
      )}

      {inspection.validation.diagnostics.length > 0 && (
        <div className="mt-3 space-y-2">
          {inspection.validation.diagnostics.map((diagnostic, index) => (
            <div
              key={`${diagnostic.code}-${diagnostic.path ?? "root"}-${index}`}
              className={cn(
                "rounded-lg border px-3 py-2 text-xs",
                diagnostic.severity === "error"
                  ? "border-(--color-error)/25 bg-(--color-error-subtle) text-(--color-error)"
                  : "border-(--color-warning)/25 bg-(--color-warning)/8 text-(--color-warning)",
              )}
            >
              <span className="font-medium">{diagnostic.code}</span>
              {diagnostic.path && <span className="font-mono"> · {diagnostic.path}</span>}
              <p className="mt-0.5 leading-relaxed">{diagnostic.message}</p>
            </div>
          ))}
        </div>
      )}
    </div>
  )
}

function TemplateSummary({ kind }: { kind: ResourceKind }) {
  const copy = RESOURCE_CREATE_COPY[kind]
  const Icon = RESOURCE_CREATE_ICON[kind]
  return (
    <div className="rounded-xl border border-(--color-accent)/25 bg-(--color-accent-soft)/25 p-4">
      <div className="flex gap-3">
        <span className="grid size-9 shrink-0 place-items-center rounded-lg bg-(--color-accent-soft) text-(--color-accent)">
          <Icon className="size-4" />
        </span>
        <div className="min-w-0">
          <p className="text-sm font-medium">{copy.templateTitle}</p>
          <p className="mt-1 text-xs leading-relaxed text-(--color-text-muted)">
            {copy.templateDescription}
          </p>
        </div>
      </div>
      <div className="mt-3 flex items-center justify-between gap-3 border-t border-(--color-accent)/15 pt-3 text-xs">
        <span className="text-(--color-text-muted)">Draft source</span>
        <span className="font-mono text-(--color-text)">{copy.sourceHint}</span>
      </div>
    </div>
  )
}

function SummaryItem({ label, value, mono = false }: { label: string; value: string; mono?: boolean }) {
  return (
    <div className="rounded-lg bg-(--bg-page)/60 px-3 py-2">
      <dt className="text-[0.65rem] uppercase tracking-wide text-(--color-text-subtle)">{label}</dt>
      <dd className={cn("mt-0.5 truncate text-sm font-medium", mono && "font-mono text-xs")}>
        {value}
      </dd>
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
    <div>
      <div className="mb-1.5 flex items-center justify-between gap-2">
        <Label htmlFor={htmlFor}>{label}</Label>
        {hint && <span className="text-[0.65rem] text-(--color-text-subtle)">{hint}</span>}
      </div>
      {children}
    </div>
  )
}

function hasAllowedExtension(name: string) {
  const normalized = name.toLowerCase()
  return RESOURCE_ARCHIVE_EXTENSIONS.some((extension) => normalized.endsWith(extension))
}

function normalizeResourceSlug(kind: ResourceKind, value: string) {
  const allowed = kind === RESOURCE_KIND.AGENT ? /[^a-z0-9._-]+/g : /[^a-z0-9-]+/g
  return value
    .toLowerCase()
    .trim()
    .replace(allowed, "-")
    .replace(/^-+|-+$/g, "")
}

function titleFromSlug(slug: string) {
  return slug
    .split(/[-_]+/)
    .filter(Boolean)
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(" ")
}

function formatBytes(value: number) {
  if (value < 1024) return `${value} B`
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KiB`
  return `${(value / (1024 * 1024)).toFixed(1)} MiB`
}
