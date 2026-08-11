import { RESOURCE_KIND, type ResourceKind } from "@/shared/constants/resource"

export const RESOURCE_STUDIO_VIEW_MODE = {
  FILE: "file",
  PREVIEW: "preview",
  EDIT: "edit",
  SPLIT: "split",
  DIFF: "diff",
} as const

export type ResourceStudioViewMode =
  (typeof RESOURCE_STUDIO_VIEW_MODE)[keyof typeof RESOURCE_STUDIO_VIEW_MODE]

export const RESOURCE_STUDIO_PANEL = {
  FILES: "files",
  PROBLEMS: "problems",
  GUIDE: "guide",
} as const

export type ResourceStudioPanel =
  (typeof RESOURCE_STUDIO_PANEL)[keyof typeof RESOURCE_STUDIO_PANEL]

export const RESOURCE_STUDIO_TAB = {
  SOURCE: "source",
  VERSIONS: "versions",
  MONITORING: "monitoring",
} as const

export type ResourceStudioTab =
  (typeof RESOURCE_STUDIO_TAB)[keyof typeof RESOURCE_STUDIO_TAB]

export const RESOURCE_STUDIO_ENTRY_KIND = {
  FILE: "file",
  DIRECTORY: "directory",
} as const

export type ResourceStudioEntryKind =
  (typeof RESOURCE_STUDIO_ENTRY_KIND)[keyof typeof RESOURCE_STUDIO_ENTRY_KIND]

export const RESOURCE_STUDIO_TREE = {
  DEFAULT_WIDTH: 300,
  MIN_WIDTH: 232,
  MAX_WIDTH: 440,
  MIN_EDITOR_WIDTH: 420,
  STORAGE_KEY: "conductor.resource-studio.tree-width",
} as const

export const RESOURCE_STUDIO_LAYOUT = {
  MOBILE_MIN_HEIGHT: 720,
} as const

export const RESOURCE_STUDIO_CHANGE_KIND = {
  MODIFIED: "modified",
  ADDED: "added",
  DELETED: "deleted",
} as const

export type ResourceStudioChangeKind =
  (typeof RESOURCE_STUDIO_CHANGE_KIND)[keyof typeof RESOURCE_STUDIO_CHANGE_KIND]

export const RESOURCE_STUDIO_DIFF = {
  MAX_MATRIX_CELLS: 250_000,
} as const

export const RESOURCE_STUDIO_PREVIEW_EXTENSIONS = new Set([
  "md",
  "markdown",
  "html",
  "htm",
])

export const RESOURCE_STUDIO_LANGUAGE_BY_EXTENSION: Record<string, string> = {
  json: "json",
  jsonl: "json",
  md: "markdown",
  markdown: "markdown",
  yaml: "yaml",
  yml: "yaml",
  toml: "ini",
  py: "python",
  js: "javascript",
  jsx: "javascript",
  ts: "typescript",
  tsx: "typescript",
  css: "css",
  html: "html",
  htm: "html",
  sh: "shell",
  rs: "rust",
} as const

export const RESOURCE_STUDIO_DEFAULT_LANGUAGE = "plaintext" as const
export const RESOURCE_STUDIO_DEFAULT_NEW_FILE = "README.md" as const

export function resourceStudioExtension(path: string | null): string {
  if (!path) return ""
  const name = path.split("/").at(-1) ?? path
  const index = name.lastIndexOf(".")
  return index > 0 ? name.slice(index + 1).toLowerCase() : ""
}

export function resourceStudioLanguage(path: string | null): string {
  return (
    RESOURCE_STUDIO_LANGUAGE_BY_EXTENSION[resourceStudioExtension(path)] ??
    RESOURCE_STUDIO_DEFAULT_LANGUAGE
  )
}

export function resourceStudioCanPreview(path: string | null): boolean {
  return RESOURCE_STUDIO_PREVIEW_EXTENSIONS.has(resourceStudioExtension(path))
}

export function resourceStudioInitialContent(path: string): string {
  const extension = resourceStudioExtension(path)
  if (extension === "json" || extension === "jsonl") return "{}\n"
  if (extension === "md" || extension === "markdown") {
    const name = (path.split("/").at(-1) ?? "New file").replace(/\.[^.]+$/, "")
    return `# ${name}\n`
  }
  return ""
}

export function resourceStudioRequiredEntry(kind: ResourceKind, slug: string): string {
  if (kind === RESOURCE_KIND.PLUGIN) return "plugin.json"
  if (kind === RESOURCE_KIND.SKILL) return "SKILL.md"
  if (kind === RESOURCE_KIND.AGENT) return `${slug}.md`
  return `${slug}.json`
}
