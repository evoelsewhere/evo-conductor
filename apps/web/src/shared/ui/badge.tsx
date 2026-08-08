import type { ComponentProps } from "react"
import { cva, type VariantProps } from "class-variance-authority"

import { cn } from "@/shared/lib/utils"

const badgeVariants = cva(
  "inline-flex max-w-full items-center gap-1 rounded-sm border px-1.5 py-0.5 text-[0.7rem] leading-tight font-medium whitespace-nowrap",
  {
    variants: {
      tone: {
        neutral:
          "border-(--color-border) bg-(--bg-key) text-(--color-text-muted)",
        accent:
          "border-(--color-accent)/35 bg-(--color-accent-soft) text-(--color-accent)",
        success:
          "border-(--color-success)/35 bg-(--color-success)/12 text-(--color-success)",
        warning:
          "border-(--color-warning)/35 bg-(--color-warning)/12 text-(--color-warning)",
        danger:
          "border-(--color-error)/35 bg-(--color-error-subtle) text-(--color-error)",
        info: "border-(--accent-blue)/35 bg-(--accent-blue)/12 text-(--accent-blue-text)",
      },
    },
    defaultVariants: {
      tone: "neutral",
    },
  },
)

function Badge({
  className,
  tone,
  ...props
}: ComponentProps<"span"> & VariantProps<typeof badgeVariants>) {
  return (
    <span
      data-slot="badge"
      className={cn(badgeVariants({ tone, className }))}
      {...props}
    />
  )
}

/** Small filled dot for inline status, sized to sit on a text baseline. */
function StatusDot({
  tone = "neutral",
  className,
}: {
  tone?: "neutral" | "success" | "warning" | "danger" | "accent"
  className?: string
}) {
  const colors = {
    neutral: "bg-(--color-text-subtle)",
    success: "bg-(--color-success)",
    warning: "bg-(--color-warning)",
    danger: "bg-(--color-error)",
    accent: "bg-(--color-accent)",
  } as const

  return (
    <span
      aria-hidden="true"
      className={cn("size-1.5 shrink-0 rounded-full", colors[tone], className)}
    />
  )
}

export { Badge, badgeVariants, StatusDot }
