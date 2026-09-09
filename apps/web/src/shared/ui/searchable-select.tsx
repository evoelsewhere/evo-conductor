import { Combobox } from "@base-ui/react/combobox"
import { Check, ChevronsUpDown } from "lucide-react"

import { cn } from "@/shared/lib/utils"

export interface SearchableSelectOption {
  value: string
  label: string
}

/**
 * Single-select combobox: looks like `Select`, but opening it exposes a text
 * input that filters the option list — for pickers with enough options that
 * scanning them by eye stops being the fastest way to find one.
 */
function SearchableSelect({
  value,
  onValueChange,
  options,
  disabled,
  id,
  placeholder = "Search…",
  className,
  "aria-label": ariaLabel,
}: {
  value: string
  onValueChange: (value: string) => void
  options: readonly SearchableSelectOption[]
  disabled?: boolean
  id?: string
  placeholder?: string
  className?: string
  "aria-label"?: string
}) {
  const selected = options.find((option) => option.value === value) ?? null

  return (
    <Combobox.Root
      items={options as SearchableSelectOption[]}
      value={selected}
      onValueChange={(next) =>
        onValueChange(next ? (next as SearchableSelectOption).value : "")
      }
      isItemEqualToValue={(a: SearchableSelectOption, b: SearchableSelectOption) =>
        a.value === b.value
      }
      disabled={disabled}
    >
      <Combobox.InputGroup
        className={cn(
          "flex h-9 w-full items-center gap-2 rounded-md border border-(--color-border) bg-(--bg-page) px-2.5 transition-colors md:h-8",
          "focus-within:border-(--focus-ring) focus-within:ring-2 focus-within:ring-(--focus-ring)/25",
          "hover:border-(--color-border-strong) data-disabled:cursor-not-allowed data-disabled:opacity-60",
          className,
        )}
      >
        <Combobox.Input
          id={id}
          placeholder={placeholder}
          aria-label={ariaLabel}
          className="h-full min-w-0 flex-1 border-0 bg-transparent p-0 text-sm text-(--color-text) outline-none placeholder:text-(--color-text-subtle)"
        />
        <Combobox.Icon className="shrink-0 text-(--color-text-subtle)">
          <ChevronsUpDown className="size-3.5" />
        </Combobox.Icon>
      </Combobox.InputGroup>

      <Combobox.Portal>
        <Combobox.Positioner
          className="z-(--z-popover) outline-none"
          sideOffset={6}
        >
          <Combobox.Popup
            className={cn(
              "w-[var(--anchor-width)] max-w-[var(--available-width)] rounded-lg border border-(--border-card) bg-(--bg-card) p-1 shadow-(--shadow-depth) outline-none",
              "origin-(--transform-origin) transition-[opacity,transform] duration-(--motion-fast) data-ending-style:scale-98 data-ending-style:opacity-0 data-starting-style:scale-98 data-starting-style:opacity-0",
            )}
          >
            <Combobox.Empty className="empty:hidden px-2 py-1.5 text-xs text-(--color-text-subtle)">
              No matches.
            </Combobox.Empty>
            <Combobox.List className="max-h-[min(16rem,var(--available-height))] overflow-y-auto overscroll-contain">
              {(option: SearchableSelectOption) => (
                <Combobox.Item
                  key={option.value}
                  value={option}
                  className="flex cursor-default items-center gap-2 rounded-md px-2 py-1.5 text-sm text-(--color-text-2) outline-none select-none data-highlighted:bg-(--bg-key) data-highlighted:text-(--color-text)"
                >
                  <span className="grid size-3.5 shrink-0 place-items-center text-(--color-accent)">
                    <Combobox.ItemIndicator>
                      <Check className="size-3.5" />
                    </Combobox.ItemIndicator>
                  </span>
                  <span className="truncate">{option.label}</span>
                </Combobox.Item>
              )}
            </Combobox.List>
          </Combobox.Popup>
        </Combobox.Positioner>
      </Combobox.Portal>
    </Combobox.Root>
  )
}

export { SearchableSelect }
