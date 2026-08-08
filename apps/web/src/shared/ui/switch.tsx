import { Switch as SwitchPrimitive } from "@base-ui/react/switch"

import { cn } from "@/shared/lib/utils"

function Switch({
  checked,
  onCheckedChange,
  disabled,
  id,
  className,
}: {
  checked: boolean
  onCheckedChange: (checked: boolean) => void
  disabled?: boolean
  id?: string
  className?: string
}) {
  return (
    <SwitchPrimitive.Root
      id={id}
      checked={checked}
      onCheckedChange={onCheckedChange}
      disabled={disabled}
      className={cn(
        "relative inline-flex h-5 w-9 shrink-0 items-center rounded-full border border-(--color-border) bg-(--bg-key) p-0.5 transition-colors duration-(--motion-fast) outline-none",
        "focus-visible:ring-2 focus-visible:ring-(--focus-ring)/40",
        "data-checked:border-transparent data-checked:bg-(--color-accent)",
        "data-disabled:cursor-not-allowed data-disabled:opacity-50",
        className,
      )}
    >
      <SwitchPrimitive.Thumb className="size-4 rounded-full bg-white shadow-sm transition-transform duration-(--motion-fast) data-checked:translate-x-4" />
    </SwitchPrimitive.Root>
  )
}

/** Switch plus label and helper copy — the standard settings row. */
function SwitchField({
  id,
  label,
  description,
  checked,
  onCheckedChange,
  disabled,
}: {
  id: string
  label: string
  description?: string
  checked: boolean
  onCheckedChange: (checked: boolean) => void
  disabled?: boolean
}) {
  return (
    <div className="flex items-start justify-between gap-4">
      <div className="min-w-0">
        <label
          htmlFor={id}
          className="text-sm font-medium text-(--color-text-2) select-none"
        >
          {label}
        </label>
        {description && (
          <p className="mt-0.5 text-xs text-(--color-text-subtle)">{description}</p>
        )}
      </div>
      <Switch
        id={id}
        checked={checked}
        onCheckedChange={onCheckedChange}
        disabled={disabled}
      />
    </div>
  )
}

export { Switch, SwitchField }
