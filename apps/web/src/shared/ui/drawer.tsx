import { Dialog as DialogPrimitive } from "@base-ui/react/dialog"
import { X } from "lucide-react"
import { useEffect, useRef } from "react"

import { cn } from "@/shared/lib/utils"
import { Button } from "@/shared/ui/button"

function Drawer({
  open,
  title,
  description,
  onClose,
  children,
  footer,
  className,
}: {
  open: boolean
  title: string
  description?: string
  onClose: () => void
  children: React.ReactNode
  footer?: React.ReactNode
  className?: string
}) {
  const returnFocusRef = useRef<HTMLElement | null>(null)

  useEffect(() => {
    if (!open) return
    returnFocusRef.current =
      document.activeElement instanceof HTMLElement ? document.activeElement : null
    return () => {
      const target = returnFocusRef.current
      window.requestAnimationFrame(() => target?.focus())
    }
  }, [open])

  return (
    <DialogPrimitive.Root
      open={open}
      onOpenChange={(next) => {
        if (!next) onClose()
      }}
    >
      <DialogPrimitive.Portal>
        <DialogPrimitive.Backdrop className="fixed inset-0 z-(--z-overlay) bg-(--color-overlay) transition-opacity duration-(--motion-fast) data-ending-style:opacity-0 data-starting-style:opacity-0" />
        <div className="pointer-events-none fixed inset-0 z-(--z-modal) flex justify-end">
          <DialogPrimitive.Popup
            finalFocus={() => returnFocusRef.current}
            className={cn(
              "pointer-events-auto flex h-full w-full flex-col border-l border-(--border-card) bg-(--bg-card) shadow-(--shadow-depth) outline-none transition-transform duration-(--motion-fast) data-ending-style:translate-x-full data-starting-style:translate-x-full sm:max-w-xl",
              className,
            )}
          >
            <div className="flex items-start gap-3 border-b border-(--border-soft) px-5 py-4">
              <div className="min-w-0 flex-1">
                <DialogPrimitive.Title className="text-base font-semibold tracking-tight">
                  {title}
                </DialogPrimitive.Title>
                {description && (
                  <DialogPrimitive.Description className="mt-1 text-xs leading-relaxed text-(--color-text-muted)">
                    {description}
                  </DialogPrimitive.Description>
                )}
              </div>
              <DialogPrimitive.Close
                render={
                  <Button
                    variant="ghost"
                    size="icon"
                    aria-label="Close"
                    className="-mt-1 -mr-1"
                  />
                }
              >
                <X className="size-4" />
              </DialogPrimitive.Close>
            </div>
            <div className="min-h-0 flex-1 overflow-y-auto p-5">{children}</div>
            {footer && (
              <div className="flex flex-wrap justify-end gap-2 border-t border-(--border-soft) px-5 py-3">
                {footer}
              </div>
            )}
          </DialogPrimitive.Popup>
        </div>
      </DialogPrimitive.Portal>
    </DialogPrimitive.Root>
  )
}

export { Drawer }
