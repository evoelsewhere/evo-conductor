import { cn } from "@/shared/lib/utils"

const sizes = {
  sm: "size-6 text-[0.6rem]",
  md: "size-8 text-[0.7rem]",
} as const

/** Derives up to two initials, falling back to the first email character. */
function initialsOf(name: string, email?: string) {
  const parts = name.trim().split(/\s+/).filter(Boolean)
  if (parts.length === 0) return (email?.[0] ?? "?").toUpperCase()
  if (parts.length === 1) return parts[0].slice(0, 2).toUpperCase()
  return `${parts[0][0]}${parts[parts.length - 1][0]}`.toUpperCase()
}

function Avatar({
  name,
  email,
  size = "md",
  className,
}: {
  name: string
  email?: string
  size?: keyof typeof sizes
  className?: string
}) {
  return (
    <span
      aria-hidden="true"
      className={cn(
        "grid shrink-0 place-items-center rounded-full border border-(--color-accent)/30 bg-(--color-accent-soft) font-semibold tracking-wide text-(--color-accent)",
        sizes[size],
        className,
      )}
    >
      {initialsOf(name, email)}
    </span>
  )
}

export { Avatar }
