import {
  DEFAULT_FILE_ICON,
  FILE_ICON_BY_EXTENSION,
  FILE_ICON_BY_MIME_PREFIX,
  FILE_ICON_SPECIAL_RULES,
  FOLDER_ICON,
  FOLDER_OPEN_ICON,
  JSON_MIME_ICON,
} from "@/shared/constants/file-icon"

function baseName(path: string): string {
  return path.split(/[\\/]/).pop()?.toLowerCase() ?? path.toLowerCase()
}

function extensionOf(name: string): string {
  const index = name.lastIndexOf(".")
  return index > 0 ? name.slice(index + 1).toLowerCase() : ""
}

function iconForFile(name: string, mime: string): string {
  const lowerName = baseName(name)
  const special = FILE_ICON_SPECIAL_RULES.find(({ pattern }) => pattern.test(lowerName))
  if (special) return special.icon

  const extensionIcon = FILE_ICON_BY_EXTENSION[extensionOf(lowerName)]
  if (extensionIcon) return extensionIcon
  if (mime.includes("json")) return JSON_MIME_ICON
  return (
    FILE_ICON_BY_MIME_PREFIX.find(({ prefix }) => mime.startsWith(prefix))?.icon ??
    DEFAULT_FILE_ICON
  )
}

function IconImage({ src, size }: { src: string; size: number }) {
  return (
    <img
      src={src}
      alt=""
      width={size}
      height={size}
      draggable={false}
      aria-hidden="true"
      className="pointer-events-none block shrink-0 select-none"
    />
  )
}

export function FileTypeIcon({
  name,
  mime = "",
  size = 16,
}: {
  name: string
  mime?: string
  size?: number
}) {
  return <IconImage src={iconForFile(name, mime)} size={size} />
}

export function FolderTypeIcon({
  open = false,
  size = 16,
}: {
  open?: boolean
  size?: number
}) {
  return <IconImage src={open ? FOLDER_OPEN_ICON : FOLDER_ICON} size={size} />
}
