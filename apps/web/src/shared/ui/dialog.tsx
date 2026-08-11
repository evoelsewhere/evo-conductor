import { Dialog as DialogPrimitive } from "@base-ui/react/dialog"
import { X } from "lucide-react"
import { useEffect, useRef } from "react"

import { Button } from "@/shared/ui/button"
import { cn } from "@/shared/lib/utils"

function Dialog({
  open,
  title,
  description,
  onClose,
  children,
  footer,
  className,
  contentClassName,
}: {
  open: boolean
  title: string
  description?: string
  onClose: () => void
  children: React.ReactNode
  footer?: React.ReactNode
  className?: string
  contentClassName?: string
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
        <div className="pointer-events-none fixed inset-0 z-(--z-modal) flex items-end justify-center sm:items-center sm:p-6">
          <DialogPrimitive.Popup
            finalFocus={() => returnFocusRef.current}
            className={cn(
              "pointer-events-auto relative flex max-h-[90dvh] w-full flex-col overflow-hidden rounded-t-2xl border border-(--border-card) bg-(--bg-card) shadow-(--shadow-depth) outline-none transition-[opacity,transform] duration-(--motion-fast) data-ending-style:translate-y-3 data-ending-style:opacity-0 data-starting-style:translate-y-3 data-starting-style:opacity-0 sm:max-w-md sm:rounded-xl sm:data-ending-style:scale-98 sm:data-ending-style:translate-y-0 sm:data-starting-style:scale-98 sm:data-starting-style:translate-y-0",
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
            <div className={cn("overflow-y-auto p-5", contentClassName)}>
              {children}
            </div>
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

function ConfirmDialog({
  open,
  title,
  description,
  confirmLabel,
  busy = false,
  tone = "danger",
  onConfirm,
  onClose,
}: {
  open: boolean
  title: string
  description: string
  confirmLabel: string
  busy?: boolean
  tone?: "danger" | "default"
  onConfirm: () => void
  onClose: () => void
}) {
  return (
    <Dialog
      open={open}
      title={title}
      description={description}
      onClose={onClose}
      footer={
        <>
          <Button variant="ghost" disabled={busy} onClick={onClose}>
            Cancel
          </Button>
          <Button
            variant={tone === "danger" ? "destructive" : "default"}
            disabled={busy}
            onClick={onConfirm}
          >
            {busy ? "Working…" : confirmLabel}
          </Button>
        </>
      }
    >
      <p className="text-sm text-(--color-text-muted)">
        This action takes effect immediately.
      </p>
    </Dialog>
  )
}

export { ConfirmDialog, Dialog }
