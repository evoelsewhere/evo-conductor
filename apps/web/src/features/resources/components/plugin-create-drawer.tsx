import { useMutation } from "@tanstack/react-query"
import {
  AlertTriangle,
  CheckCircle2,
  FileArchive,
  FileCode2,
  PackagePlus,
  Sparkles,
  UploadCloud,
} from "lucide-react"
import { useEffect, useRef, useState } from "react"

import {
  api,
  type ManagedResource,
  type PluginArchiveInspection,
} from "@/shared/api/client"
import {
  PLUGIN_ARCHIVE_ACCEPT,
  PLUGIN_ARCHIVE_EXTENSIONS,
  PLUGIN_ARCHIVE_MAX_BYTES,
  PLUGIN_CREATE_MODE,
  PLUGIN_IMPORT_CHANGELOG,
  PLUGIN_INITIAL_VERSION,
  RESOURCE_KIND,
  RESOURCE_VISIBILITY_OPTIONS,
  type PluginCreateMode,
} from "@/shared/constants/resource"
import { cn } from "@/shared/lib/utils"
import { Badge } from "@/shared/ui/badge"
import { Button } from "@/shared/ui/button"
import { Drawer } from "@/shared/ui/drawer"
import { ErrorState } from "@/shared/ui/empty-state"
import { Input } from "@/shared/ui/input"
import { Label } from "@/shared/ui/label"
import { Select } from "@/shared/ui/select"
import { Textarea } from "@/shared/ui/textarea"

type Visibility = ManagedResource["visibility"]

export function PluginCreateDrawer({
  open,
  onClose,
  onCreated,
}: {
  open: boolean
  onClose: () => void
  onCreated: (resource: ManagedResource) => void
}) {
  const fileInputRef = useRef<HTMLInputElement | null>(null)
  const selectedFileRef = useRef<File | null>(null)
  const [mode, setMode] = useState<PluginCreateMode>(PLUGIN_CREATE_MODE.UPLOAD)
  const [file, setFile] = useState<File | null>(null)
  const [inspection, setInspection] = useState<PluginArchiveInspection | null>(null)
  const [inspectError, setInspectError] = useState<string | null>(null)
  const [localError, setLocalError] = useState<string | null>(null)
  const [dragActive, setDragActive] = useState(false)
  const [name, setName] = useState("")
  const [slug, setSlug] = useState("")
  const [description, setDescription] = useState("")
  const [visibility, setVisibility] = useState<Visibility>("shared")

  const inspect = useMutation({
    mutationFn: (archive: File) => api.inspectPluginArchive(archive),
    onSuccess: (result, archive) => {
      if (selectedFileRef.current !== archive) return
      setInspection(result)
      setInspectError(null)
      if (result.manifest.name) {
        setName((current) => current || titleFromSlug(result.manifest.name!))
      }
    },
    onError: (error, archive) => {
      if (selectedFileRef.current !== archive) return
      setInspection(null)
      setInspectError(error instanceof Error ? error.message : "Package inspection failed")
    },
  })

  const create = useMutation({
    mutationFn: async () => {
      if (mode === PLUGIN_CREATE_MODE.UPLOAD) {
        if (!file || !inspection?.validation.valid) {
          throw new Error("Choose a valid plugin package first")
        }
        const result = await api.createPluginFromArchive(file, { name, visibility })
        return result.resource
      }
      return api.createResource({
        kind: RESOURCE_KIND.PLUGIN,
        slug,
        name,
        description,
        version: PLUGIN_INITIAL_VERSION,
        visibility,
        payload: {},
        changelog: PLUGIN_IMPORT_CHANGELOG,
      })
    },
    onSuccess: onCreated,
  })

  useEffect(() => {
    if (open) return
    selectedFileRef.current = null
    setMode(PLUGIN_CREATE_MODE.UPLOAD)
    setFile(null)
    setInspection(null)
    setInspectError(null)
    setLocalError(null)
    setDragActive(false)
    setName("")
    setSlug("")
    setDescription("")
    setVisibility("shared")
    inspect.reset()
    create.reset()
  }, [open])

  const dirty =
    file !== null ||
    name.trim() !== "" ||
    slug.trim() !== "" ||
    description.trim() !== "" ||
    visibility !== "shared"

  function requestClose() {
    if (create.isPending || inspect.isPending) return
    if (dirty && !window.confirm("Discard this plugin draft setup?")) return
    onClose()
  }

  function chooseFile(nextFile: File | undefined) {
    if (!nextFile) return
    setLocalError(null)
    setInspectError(null)
    setInspection(null)
    if (!hasAllowedExtension(nextFile.name)) {
      setLocalError("Choose a .zip or .evoplugin package.")
      return
    }
    if (nextFile.size === 0 || nextFile.size > PLUGIN_ARCHIVE_MAX_BYTES) {
      setLocalError("Plugin packages must be between 1 byte and 20 MiB.")
      return
    }
    selectedFileRef.current = nextFile
    setFile(nextFile)
    setName("")
    inspect.mutate(nextFile)
  }

  const createDisabled =
    create.isPending ||
    !name.trim() ||
    (mode === PLUGIN_CREATE_MODE.UPLOAD
      ? !file || inspect.isPending || !inspection?.validation.valid
      : !slug.trim())

  return (
    <Drawer
      open={open}
      title="Add plugin"
      description="Import an existing package or start from an editable EvoFlux template."
      onClose={requestClose}
      footer={
        <>
          <Button variant="ghost" onClick={requestClose} disabled={create.isPending || inspect.isPending}>
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
            <PackagePlus className="size-3.5" />
            {create.isPending ? "Creating…" : "Create plugin draft"}
          </Button>
        </>
      }
    >
      <div className="space-y-5">
        <CreateModeSelector value={mode} onChange={setMode} />

        {(localError || inspectError || create.error) && (
          <ErrorState
            message={
              localError ??
              inspectError ??
              (create.error instanceof Error ? create.error.message : "Plugin creation failed")
            }
          />
        )}

        {mode === PLUGIN_CREATE_MODE.UPLOAD ? (
          <>
            <input
              ref={fileInputRef}
              type="file"
              className="sr-only"
              accept={PLUGIN_ARCHIVE_ACCEPT}
              onChange={(event) => {
                chooseFile(event.target.files?.[0])
                event.target.value = ""
              }}
            />
            <div
              className={cn(
                "rounded-xl border border-dashed p-6 text-center transition-colors",
                dragActive
                  ? "border-(--color-accent) bg-(--color-accent-soft)/50"
                  : "border-(--color-border) bg-(--bg-page)/45",
              )}
              onDragEnter={(event) => {
                event.preventDefault()
                setDragActive(true)
              }}
              onDragOver={(event) => event.preventDefault()}
              onDragLeave={(event) => {
                if (!event.currentTarget.contains(event.relatedTarget as Node | null)) {
                  setDragActive(false)
                }
              }}
              onDrop={(event) => {
                event.preventDefault()
                setDragActive(false)
                chooseFile(event.dataTransfer.files[0])
              }}
            >
              <span className="mx-auto grid size-11 place-items-center rounded-xl border border-(--border-soft) bg-(--bg-key) text-(--color-accent)">
                <UploadCloud className="size-5" />
              </span>
              <p className="mt-3 text-sm font-medium">Drop a plugin package here</p>
              <p className="mt-1 text-xs text-(--color-text-muted)">
                ZIP or EVOPLUGIN · editable UTF-8 files · up to 20 MiB
              </p>
              <Button
                type="button"
                variant="outline"
                size="sm"
                className="mt-4"
                onClick={() => fileInputRef.current?.click()}
                disabled={inspect.isPending}
              >
                <FileArchive className="size-3.5" />
                Browse package
              </Button>
            </div>

            {file && (
              <div className="flex items-center gap-3 rounded-lg border border-(--border-soft) bg-(--bg-key)/60 px-3 py-2.5">
                <FileArchive className="size-4 shrink-0 text-(--color-accent)" />
                <div className="min-w-0 flex-1">
                  <p className="truncate text-sm font-medium">{file.name}</p>
                  <p className="text-xs text-(--color-text-subtle)">{formatBytes(file.size)}</p>
                </div>
                {inspect.isPending ? (
                  <Badge>Inspecting…</Badge>
                ) : inspection?.validation.valid ? (
                  <Badge className="border-(--color-success)/30 text-(--color-success)">
                    Valid
                  </Badge>
                ) : (
                  <Badge className="border-(--color-error)/30 text-(--color-error)">
                    Needs fixes
                  </Badge>
                )}
              </div>
            )}

            {inspection && <PackageInspection inspection={inspection} />}
          </>
        ) : (
          <div className="rounded-xl border border-(--color-accent)/25 bg-(--color-accent-soft)/25 p-4">
            <div className="flex gap-3">
              <span className="grid size-9 shrink-0 place-items-center rounded-lg bg-(--color-accent-soft) text-(--color-accent)">
                <Sparkles className="size-4" />
              </span>
              <div>
                <p className="text-sm font-medium">Start with the EvoFlux plugin template</p>
                <p className="mt-1 text-xs leading-relaxed text-(--color-text-muted)">
                  Conductor creates plugin.json and a starter skill. Continue editing every file in Resource Studio.
                </p>
              </div>
            </div>
          </div>
        )}

        {(mode === PLUGIN_CREATE_MODE.TEMPLATE || inspection) && (
          <div className="space-y-4 border-t border-(--border-soft) pt-5">
            <div className="grid gap-4 sm:grid-cols-2">
              <Field label="Display name" htmlFor="plugin-display-name">
                <Input
                  id="plugin-display-name"
                  value={name}
                  maxLength={120}
                  onChange={(event) => setName(event.target.value)}
                  autoFocus={mode === PLUGIN_CREATE_MODE.TEMPLATE}
                />
              </Field>
              {mode === PLUGIN_CREATE_MODE.UPLOAD ? (
                <Field label="Manifest name" htmlFor="plugin-manifest-name" hint="Controlled by plugin.json">
                  <Input
                    id="plugin-manifest-name"
                    value={inspection?.manifest.name ?? "Unavailable"}
                    readOnly
                    className="font-mono text-xs"
                  />
                </Field>
              ) : (
                <Field label="Plugin slug" htmlFor="plugin-slug" hint="Lowercase letters, numbers and hyphens">
                  <Input
                    id="plugin-slug"
                    value={slug}
                    maxLength={80}
                    onChange={(event) =>
                      setSlug(event.target.value.toLowerCase().replace(/[^a-z0-9-]/g, "-"))
                    }
                  />
                </Field>
              )}
            </div>

            {mode === PLUGIN_CREATE_MODE.TEMPLATE && (
              <Field label="Description" htmlFor="plugin-description">
                <Textarea
                  id="plugin-description"
                  value={description}
                  maxLength={1000}
                  onChange={(event) => setDescription(event.target.value)}
                />
              </Field>
            )}

            <Field label="Default visibility" htmlFor="plugin-visibility">
              <Select
                id="plugin-visibility"
                value={visibility}
                onValueChange={setVisibility}
                options={RESOURCE_VISIBILITY_OPTIONS}
              />
            </Field>

            <div className="flex gap-2 rounded-lg border border-(--border-soft) bg-(--bg-page)/55 px-3 py-2.5 text-xs leading-relaxed text-(--color-text-muted)">
              <AlertTriangle className="mt-0.5 size-3.5 shrink-0 text-(--color-warning)" />
              <span>
                Conductor performs static package checks only. EvoFlux still requires a local trust review before enabling imported plugin capabilities.
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
}: {
  value: PluginCreateMode
  onChange: (value: PluginCreateMode) => void
}) {
  const options = [
    {
      value: PLUGIN_CREATE_MODE.UPLOAD,
      icon: UploadCloud,
      title: "Upload package",
      description: "Fastest for an existing plugin",
    },
    {
      value: PLUGIN_CREATE_MODE.TEMPLATE,
      icon: FileCode2,
      title: "Blank template",
      description: "Build in Resource Studio",
    },
  ] as const

  return (
    <div className="grid grid-cols-2 gap-2" aria-label="Plugin creation method">
      {options.map((option) => {
        const Icon = option.icon
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
            <Icon className={cn("size-4", active ? "text-(--color-accent)" : "text-(--color-text-subtle)")} />
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

function PackageInspection({ inspection }: { inspection: PluginArchiveInspection }) {
  return (
    <div className="rounded-xl border border-(--border-soft) p-4">
      <div className="flex items-center justify-between gap-3">
        <div>
          <p className="text-sm font-medium">Package inspection</p>
          <p className="mt-0.5 text-xs text-(--color-text-muted)">
            Parsed safely without executing package code.
          </p>
        </div>
        {inspection.validation.valid ? (
          <CheckCircle2 className="size-5 text-(--color-success)" aria-label="Package is valid" />
        ) : (
          <AlertTriangle className="size-5 text-(--color-error)" aria-label="Package has errors" />
        )}
      </div>

      <dl className="mt-4 grid grid-cols-2 gap-2">
        <SummaryItem label="Version" value={inspection.manifest.version ?? "Missing"} mono />
        <SummaryItem label="Files" value={String(inspection.file_count)} />
        <SummaryItem label="Skills" value={String(inspection.skill_count)} />
        <SummaryItem label="Extracted size" value={formatBytes(inspection.total_uncompressed_bytes)} />
      </dl>

      {inspection.manifest.description && (
        <p className="mt-3 rounded-lg bg-(--bg-page)/55 px-3 py-2 text-xs leading-relaxed text-(--color-text-muted)">
          {inspection.manifest.description}
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

function SummaryItem({ label, value, mono = false }: { label: string; value: string; mono?: boolean }) {
  return (
    <div className="rounded-lg bg-(--bg-page)/60 px-3 py-2">
      <dt className="text-[0.65rem] uppercase tracking-wide text-(--color-text-subtle)">{label}</dt>
      <dd className={cn("mt-0.5 truncate text-sm font-medium", mono && "font-mono text-xs")}>{value}</dd>
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
  return PLUGIN_ARCHIVE_EXTENSIONS.some((extension) => normalized.endsWith(extension))
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
