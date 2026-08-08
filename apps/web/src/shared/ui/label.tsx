import * as React from "react"

import { cn } from "@/shared/lib/utils"

function Label({ className, ...props }: React.ComponentProps<"label">) {
  return (
    <label
      className={cn(
        "text-[0.8rem] font-medium text-(--color-text-2)",
        className,
      )}
      {...props}
    />
  )
}

export { Label }
