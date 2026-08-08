import { Tooltip as TooltipPrimitive } from "@base-ui/react/tooltip"
import type { ReactElement, ReactNode } from "react"

import { cn } from "@/shared/lib/utils"

const TooltipProvider = TooltipPrimitive.Provider

/**
 * `children` must be a single element: Base UI attaches the trigger props to it
 * via `render` rather than adding a wrapper node.
 */
function Tooltip({
  content,
  children,
  side = "right",
  disabled = false,
  className,
}: {
  content: ReactNode
  children: ReactElement
  side?: "top" | "right" | "bottom" | "left"
  disabled?: boolean
  className?: string
}) {
  if (disabled || !content) return children

  return (
    <TooltipPrimitive.Root>
      <TooltipPrimitive.Trigger render={children} />
      <TooltipPrimitive.Portal>
        <TooltipPrimitive.Positioner side={side} sideOffset={8}>
          <TooltipPrimitive.Popup
            className={cn(
              "z-(--z-toast) rounded-md border border-(--border-card) bg-(--bg-card) px-2 py-1 text-xs font-medium text-(--color-text) shadow-(--shadow-depth)",
              "origin-(--transform-origin) transition-[opacity,transform] duration-(--motion-fast) data-ending-style:scale-95 data-ending-style:opacity-0 data-starting-style:scale-95 data-starting-style:opacity-0",
              className,
            )}
          >
            {content}
          </TooltipPrimitive.Popup>
        </TooltipPrimitive.Positioner>
      </TooltipPrimitive.Portal>
    </TooltipPrimitive.Root>
  )
}

export { Tooltip, TooltipProvider }
