import type { ComponentProps } from "react"

import { cn } from "@/shared/lib/utils"

function Badge({ className, ...props }: ComponentProps<"span">) {
  return (
    <span
      className={cn(
        "inline-flex items-center rounded-sm border border-(--color-border) bg-(--bg-key) px-1.5 py-0.5 text-[0.7rem] font-medium text-(--color-text-muted)",
        className,
      )}
      {...props}
    />
  )
}

export { Badge }
