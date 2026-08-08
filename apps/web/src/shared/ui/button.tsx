import { Button as ButtonPrimitive } from "@base-ui/react/button"
import { cva, type VariantProps } from "class-variance-authority"

import { cn } from "@/shared/lib/utils"

const buttonVariants = cva(
  "group/button inline-flex shrink-0 items-center justify-center rounded-md border border-transparent bg-clip-padding text-sm font-medium whitespace-nowrap text-(--color-text) transition-[background-color,border-color,color,box-shadow,transform,opacity] duration-(--motion-fast) outline-none select-none focus-visible:ring-2 focus-visible:ring-(--focus-ring)/40 active:not-aria-[haspopup]:translate-y-px disabled:pointer-events-none disabled:opacity-50 [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-4",
  {
    variants: {
      variant: {
        default:
          "border-(--color-accent) bg-(--color-accent) text-(--color-text-on-accent) hover:opacity-90",
        gradient:
          "border-transparent bg-gradient-primary text-(--color-text-on-accent) hover:opacity-90 shadow-[0_2px_8px_rgba(102,126,234,0.35)]",
        outline:
          "border-(--color-border) bg-(--bg-page) hover:border-(--color-border-strong) hover:bg-(--bg-key)",
        secondary:
          "border-(--color-border) bg-(--bg-key) text-(--color-text) hover:bg-(--color-surface-2)",
        ghost: "bg-transparent hover:bg-(--bg-key) hover:text-(--color-text)",
        destructive:
          "border-(--color-error)/30 bg-(--color-error-subtle) text-(--color-error) hover:bg-(--color-error)/15",
      },
      size: {
        default: "h-9 gap-1.5 px-2.5 md:h-8",
        sm: "h-8 gap-1 rounded-sm px-2 text-[0.8rem] md:h-7",
        lg: "h-10 gap-1.5 px-3 md:h-9",
        icon: "size-9 md:size-8",
      },
    },
    defaultVariants: {
      variant: "default",
      size: "default",
    },
  },
)

function Button({
  className,
  variant = "default",
  size = "default",
  ...props
}: ButtonPrimitive.Props & VariantProps<typeof buttonVariants>) {
  return (
    <ButtonPrimitive
      data-slot="button"
      className={cn(buttonVariants({ variant, size, className }))}
      {...props}
    />
  )
}

export { Button, buttonVariants }
