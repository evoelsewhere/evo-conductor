import { Select as SelectPrimitive } from "@base-ui/react/select"
import { Check, ChevronsUpDown } from "lucide-react"

import { cn } from "@/shared/lib/utils"

export interface SelectOption<T extends string> {
  value: T
  label: string
}

function Select<T extends string>({
  value,
  onValueChange,
  options,
  disabled,
  id,
  placeholder = "Select…",
  className,
  "aria-label": ariaLabel,
}: {
  value: T
  onValueChange: (value: T) => void
  options: readonly SelectOption<T>[]
  disabled?: boolean
  id?: string
  placeholder?: string
  className?: string
  "aria-label"?: string
}) {
  return (
    <SelectPrimitive.Root
      items={options}
      value={value}
      onValueChange={(next) => {
        if (next != null) onValueChange(next as T)
      }}
      disabled={disabled}
    >
      <SelectPrimitive.Trigger
        id={id}
        aria-label={ariaLabel}
        className={cn(
          "flex h-9 w-full items-center justify-between gap-2 rounded-md border border-(--color-border) bg-(--bg-page) px-2.5 text-sm text-(--color-text) transition-colors outline-none select-none md:h-8",
          "hover:border-(--color-border-strong) focus-visible:border-(--focus-ring) focus-visible:ring-2 focus-visible:ring-(--focus-ring)/25",
          "data-disabled:cursor-not-allowed data-disabled:opacity-60",
          className,
        )}
      >
        <SelectPrimitive.Value
          className="truncate data-placeholder:text-(--color-text-subtle)"
          placeholder={placeholder}
        />
        <SelectPrimitive.Icon className="shrink-0 text-(--color-text-subtle)">
          <ChevronsUpDown className="size-3.5" />
        </SelectPrimitive.Icon>
      </SelectPrimitive.Trigger>

      <SelectPrimitive.Portal>
        <SelectPrimitive.Positioner
          className="z-(--z-popover) outline-none"
          sideOffset={6}
          alignItemWithTrigger={false}
        >
          <SelectPrimitive.Popup
            className={cn(
              "min-w-[var(--anchor-width)] rounded-lg border border-(--border-card) bg-(--bg-card) p-1 shadow-(--shadow-depth) outline-none",
              "origin-(--transform-origin) transition-[opacity,transform] duration-(--motion-fast) data-ending-style:scale-98 data-ending-style:opacity-0 data-starting-style:scale-98 data-starting-style:opacity-0",
            )}
          >
            <SelectPrimitive.List className="max-h-[min(18rem,var(--available-height))] overflow-y-auto">
              {options.map((option) => (
                <SelectPrimitive.Item
                  key={option.value}
                  value={option.value}
                  className="flex cursor-default items-center gap-2 rounded-md px-2 py-1.5 text-sm text-(--color-text-2) outline-none select-none data-highlighted:bg-(--bg-key) data-highlighted:text-(--color-text)"
                >
                  {/* Fixed-width slot keeps labels aligned whether or not the check shows. */}
                  <span className="grid size-3.5 shrink-0 place-items-center text-(--color-accent)">
                    <SelectPrimitive.ItemIndicator>
                      <Check className="size-3.5" />
                    </SelectPrimitive.ItemIndicator>
                  </span>
                  <SelectPrimitive.ItemText className="truncate">
                    {option.label}
                  </SelectPrimitive.ItemText>
                </SelectPrimitive.Item>
              ))}
            </SelectPrimitive.List>
          </SelectPrimitive.Popup>
        </SelectPrimitive.Positioner>
      </SelectPrimitive.Portal>
    </SelectPrimitive.Root>
  )
}

export { Select }
