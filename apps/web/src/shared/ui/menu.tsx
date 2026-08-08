import { Menu as MenuPrimitive } from "@base-ui/react/menu"
import { Check } from "lucide-react"
import type { ComponentProps, ReactElement, ReactNode } from "react"

import { cn } from "@/shared/lib/utils"

const itemClass =
  "flex w-full cursor-default items-center gap-2 rounded-md px-2 py-1.5 text-sm text-(--color-text-2) outline-none select-none data-highlighted:bg-(--bg-key) data-highlighted:text-(--color-text) data-disabled:opacity-50"

const MenuRoot = MenuPrimitive.Root
const MenuGroup = MenuPrimitive.Group

/**
 * Styled popup for a `Menu`. `trigger` must be a single element: Base UI
 * attaches trigger props via `render` instead of adding a wrapper node.
 */
function Menu({
  trigger,
  children,
  side = "top",
  align = "start",
  className,
}: {
  trigger: ReactElement
  children: ReactNode
  side?: "top" | "right" | "bottom" | "left"
  align?: "start" | "center" | "end"
  className?: string
}) {
  return (
    <MenuPrimitive.Root>
      <MenuPrimitive.Trigger render={trigger} />
      <MenuPrimitive.Portal>
        <MenuPrimitive.Positioner
          className="z-(--z-popover) outline-none"
          side={side}
          align={align}
          sideOffset={8}
        >
          <MenuPrimitive.Popup
            className={cn(
              "min-w-56 rounded-lg border border-(--border-card) bg-(--bg-card) p-1 shadow-(--shadow-depth) outline-none",
              "origin-(--transform-origin) transition-[opacity,transform] duration-(--motion-fast) data-ending-style:scale-98 data-ending-style:opacity-0 data-starting-style:scale-98 data-starting-style:opacity-0",
              className,
            )}
          >
            {children}
          </MenuPrimitive.Popup>
        </MenuPrimitive.Positioner>
      </MenuPrimitive.Portal>
    </MenuPrimitive.Root>
  )
}

function MenuItem({
  className,
  tone = "default",
  ...props
}: ComponentProps<typeof MenuPrimitive.Item> & {
  tone?: "default" | "danger"
}) {
  return (
    <MenuPrimitive.Item
      className={cn(
        itemClass,
        tone === "danger" &&
          "text-(--color-error) data-highlighted:bg-(--color-error-subtle) data-highlighted:text-(--color-error)",
        className,
      )}
      {...props}
    />
  )
}

function MenuRadioGroup(
  props: ComponentProps<typeof MenuPrimitive.RadioGroup>,
) {
  return <MenuPrimitive.RadioGroup {...props} />
}

function MenuRadioItem({
  className,
  children,
  ...props
}: ComponentProps<typeof MenuPrimitive.RadioItem>) {
  return (
    <MenuPrimitive.RadioItem className={cn(itemClass, className)} {...props}>
      {children}
      {/* Indicator sits last so the check aligns to the trailing edge. */}
      <span className="ml-auto grid size-3.5 shrink-0 place-items-center text-(--color-accent)">
        <MenuPrimitive.RadioItemIndicator>
          <Check className="size-3.5" />
        </MenuPrimitive.RadioItemIndicator>
      </span>
    </MenuPrimitive.RadioItem>
  )
}

function MenuSeparator({
  className,
  ...props
}: ComponentProps<typeof MenuPrimitive.Separator>) {
  return (
    <MenuPrimitive.Separator
      className={cn("-mx-1 my-1 h-px bg-(--border-soft)", className)}
      {...props}
    />
  )
}

function MenuGroupLabel({
  className,
  ...props
}: ComponentProps<typeof MenuPrimitive.GroupLabel>) {
  return (
    <MenuPrimitive.GroupLabel
      className={cn(
        "px-2 pt-1.5 pb-1 text-[0.65rem] font-medium tracking-wider text-(--color-text-subtle) uppercase",
        className,
      )}
      {...props}
    />
  )
}

export {
  Menu,
  MenuRoot,
  MenuItem,
  MenuGroup,
  MenuGroupLabel,
  MenuRadioGroup,
  MenuRadioItem,
  MenuSeparator,
}
