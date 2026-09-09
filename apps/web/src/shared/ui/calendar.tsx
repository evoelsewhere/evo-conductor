import { ChevronLeft, ChevronRight } from "lucide-react"
import type { ComponentProps } from "react"
import { DayPicker } from "react-day-picker"

import { cn } from "@/shared/lib/utils"
import { buttonVariants } from "@/shared/ui/button"

/** Re-skinned `react-day-picker` — a real calendar grid, not a native date input. */
function Calendar({
  className,
  classNames,
  showOutsideDays = true,
  ...props
}: ComponentProps<typeof DayPicker>) {
  return (
    <DayPicker
      showOutsideDays={showOutsideDays}
      className={cn("p-1", className)}
      classNames={{
        months: "flex flex-col gap-3 sm:flex-row",
        month: "flex flex-col gap-3",
        month_caption: "flex items-center justify-center pt-1 relative",
        caption_label: "text-sm font-medium text-(--color-text)",
        nav: "flex items-center justify-between absolute inset-x-0 top-0",
        button_previous: cn(
          buttonVariants({ variant: "ghost", size: "icon" }),
          "size-7 text-(--color-text-subtle)",
        ),
        button_next: cn(
          buttonVariants({ variant: "ghost", size: "icon" }),
          "size-7 text-(--color-text-subtle)",
        ),
        month_grid: "w-full border-collapse",
        weekdays: "flex",
        weekday: "w-8 text-center text-[0.7rem] font-medium text-(--color-text-subtle)",
        week: "flex w-full",
        day: "size-8 p-0 text-center text-sm relative",
        day_button: cn(
          "size-8 rounded-md p-0 font-normal text-(--color-text) transition-colors",
          "hover:bg-(--bg-key) focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-(--focus-ring)/35",
        ),
        range_start: "rounded-l-md bg-(--color-accent-soft)",
        range_middle: "bg-(--color-accent-soft)",
        range_end: "rounded-r-md bg-(--color-accent-soft)",
        selected: "[&>button]:bg-(--color-accent) [&>button]:text-white [&>button]:hover:bg-(--color-accent)",
        today: "[&>button]:border [&>button]:border-(--color-accent)/40",
        outside: "text-(--color-text-subtle)/50",
        disabled: "text-(--color-text-subtle)/30 line-through",
        hidden: "invisible",
        ...classNames,
      }}
      components={{
        Chevron: ({ orientation }) =>
          orientation === "left" ? (
            <ChevronLeft className="size-4" />
          ) : (
            <ChevronRight className="size-4" />
          ),
      }}
      {...props}
    />
  )
}

export { Calendar }
