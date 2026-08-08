import { Combobox } from "@base-ui/react/combobox"
import { Check, X } from "lucide-react"
import { useMemo } from "react"

import { cn } from "@/shared/lib/utils"

export interface MultiSelectOption {
  value: string
  label: string
}

/**
 * Searchable multi-select. Selections render as removable chips inside the
 * field; the option list stays filterable so it scales past a handful of
 * entries. The `{ value, label }` item shape lets Base UI derive chip text
 * without an `itemToStringLabel` mapper.
 */
function MultiSelect({
  options,
  value,
  onChange,
  id,
  placeholder = "Search…",
  emptyLabel = "Nothing to choose from yet.",
  disabled,
  className,
}: {
  options: readonly MultiSelectOption[]
  value: readonly string[]
  onChange: (next: string[]) => void
  id?: string
  placeholder?: string
  emptyLabel?: string
  disabled?: boolean
  className?: string
}) {
  const selected = useMemo(
    () => options.filter((option) => value.includes(option.value)),
    [options, value],
  )

  if (options.length === 0) {
    return (
      <p className="rounded-md border border-dashed border-(--color-border) px-2.5 py-2 text-xs text-(--color-text-subtle)">
        {emptyLabel}
      </p>
    )
  }

  return (
    <Combobox.Root
      items={options as MultiSelectOption[]}
      multiple
      disabled={disabled}
      value={selected}
      onValueChange={(next) =>
        onChange((next as MultiSelectOption[]).map((option) => option.value))
      }
      isItemEqualToValue={(a: MultiSelectOption, b: MultiSelectOption) =>
        a.value === b.value
      }
    >
      <Combobox.InputGroup
        className={cn(
          "flex w-full cursor-text flex-wrap items-center gap-1 rounded-md border border-(--color-border) bg-(--bg-page) px-2 py-1.5 transition-colors",
          "focus-within:border-(--focus-ring) focus-within:ring-2 focus-within:ring-(--focus-ring)/25",
          "data-disabled:cursor-not-allowed data-disabled:opacity-60",
          className,
        )}
      >
        {/* Cap the chip area so a member with many tags can't grow the form. */}
        <Combobox.Chips className="flex max-h-24 w-full flex-wrap items-center gap-1 overflow-y-auto">
          <Combobox.Value>
            {(items: MultiSelectOption[]) => (
              <>
                {items.map((item) => (
                  <Combobox.Chip
                    key={item.value}
                    aria-label={item.label}
                    className="flex items-center gap-1 rounded-sm border border-(--color-accent)/35 bg-(--color-accent-soft) py-0.5 pr-1 pl-1.5 text-[0.7rem] leading-tight font-medium text-(--color-accent) outline-none focus-within:ring-2 focus-within:ring-(--focus-ring)/25 data-highlighted:ring-2 data-highlighted:ring-(--focus-ring)/25"
                  >
                    {item.label}
                    <Combobox.ChipRemove
                      aria-label={`Remove ${item.label}`}
                      className="grid size-3.5 place-items-center rounded-sm text-current opacity-70 hover:bg-(--color-accent)/15 hover:opacity-100"
                    >
                      <X className="size-3" />
                    </Combobox.ChipRemove>
                  </Combobox.Chip>
                ))}
                <Combobox.Input
                  id={id}
                  placeholder={items.length > 0 ? "" : placeholder}
                  className="h-5 min-w-16 flex-1 border-0 bg-transparent p-0 text-sm text-(--color-text) outline-none placeholder:text-(--color-text-subtle)"
                />
              </>
            )}
          </Combobox.Value>
        </Combobox.Chips>
        <Combobox.Clear
          aria-label="Clear selection"
          className="grid size-5 shrink-0 place-items-center rounded-sm text-(--color-text-subtle) hover:bg-(--bg-key) hover:text-(--color-text)"
        >
          <X className="size-3.5" />
        </Combobox.Clear>
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
              {(option: MultiSelectOption) => (
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

export { MultiSelect }
