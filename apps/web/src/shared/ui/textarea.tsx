import * as React from "react"

import { cn } from "@/shared/lib/utils"

function Textarea({ className, ...props }: React.ComponentProps<"textarea">) {
  return (
    <textarea
      className={cn(
        "min-h-20 w-full rounded-md border border-(--color-border) bg-(--bg-page) px-2.5 py-2 text-sm text-(--color-text) outline-none placeholder:text-(--color-text-subtle) hover:border-(--color-border-strong) focus-visible:border-(--focus-ring) focus-visible:ring-2 focus-visible:ring-(--focus-ring)/25",
        className,
      )}
      {...props}
    />
  )
}

export { Textarea }
