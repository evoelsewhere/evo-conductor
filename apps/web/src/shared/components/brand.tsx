import { Radio } from "lucide-react"

import { cn } from "@/shared/lib/utils"

export function BrandMark({ className }: { className?: string }) {
  return (
    <div className={cn("flex items-center gap-2.5", className)}>
      <div className="flex size-8 items-center justify-center rounded-md bg-gradient-primary text-(--color-text-on-accent) shadow-[0_2px_8px_rgba(102,126,234,0.35)]">
        <Radio className="size-4" strokeWidth={1.75} />
      </div>
      <div className="leading-tight">
        <div className="text-sm font-semibold tracking-tight text-(--color-text)">
          Evo Conductor
        </div>
        <div className="text-[0.7rem] text-(--color-text-subtle)">
          master control for EvoFlux
        </div>
      </div>
    </div>
  )
}
