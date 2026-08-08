import { Loader2 } from "lucide-react"

import { cn } from "@/shared/lib/utils"

function Spinner({ className }: { className?: string }) {
  return (
    <Loader2
      aria-hidden="true"
      className={cn("size-4 animate-spin text-(--color-text-subtle)", className)}
      strokeWidth={2}
    />
  )
}

export { Spinner }
