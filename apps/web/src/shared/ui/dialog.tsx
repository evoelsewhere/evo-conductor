import { X } from "lucide-react"

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
}: {
  open: boolean
  title: string
  description?: string
  onClose: () => void
  children: React.ReactNode
  footer?: React.ReactNode
  className?: string
}) {
  if (!open) return null

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label={title}
      className="fixed inset-0 z-(--z-modal) flex items-end justify-center sm:items-center sm:p-6"
    >
      <button
        type="button"
        aria-label="Close dialog"
        className="absolute inset-0 bg-(--color-overlay)"
        onClick={onClose}
      />
      <div
        className={cn(
          "relative z-10 flex max-h-[90dvh] w-full flex-col overflow-hidden rounded-t-2xl border border-(--border-card) bg-(--bg-card) shadow-(--shadow-depth) sm:max-w-md sm:rounded-xl",
          className,
        )}
      >
        <div className="flex items-start gap-3 border-b border-(--border-soft) px-5 py-4">
          <div className="min-w-0 flex-1">
            <h2 className="text-base font-semibold tracking-tight">{title}</h2>
            {description && (
              <p className="mt-1 text-xs leading-relaxed text-(--color-text-muted)">
                {description}
              </p>
            )}
          </div>
          <Button
            variant="ghost"
            size="icon"
            aria-label="Close"
            className="-mt-1 -mr-1"
            onClick={onClose}
          >
            <X className="size-4" />
          </Button>
        </div>
        <div className="overflow-y-auto p-5">{children}</div>
        {footer && (
          <div className="flex justify-end gap-2 border-t border-(--border-soft) px-5 py-3">
            {footer}
          </div>
        )}
      </div>
    </div>
  )
}

export { Dialog }
