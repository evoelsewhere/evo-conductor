import type { ComponentProps } from "react"

import { cn } from "@/shared/lib/utils"

/**
 * Wraps the table in its own scroll container so narrow viewports pan
 * horizontally instead of squashing or clipping columns.
 */
function TableWrap({ className, ...props }: ComponentProps<"div">) {
  return (
    <div
      data-slot="table-wrap"
      className={cn(
        "w-full overflow-x-auto overscroll-x-contain rounded-xl border border-(--border-card) bg-(--bg-card)",
        className,
      )}
      {...props}
    />
  )
}

function Table({ className, ...props }: ComponentProps<"table">) {
  return (
    <table
      data-slot="table"
      className={cn("w-full min-w-max text-left text-sm", className)}
      {...props}
    />
  )
}

function TableHead({ className, ...props }: ComponentProps<"thead">) {
  return (
    <thead
      data-slot="table-head"
      className={cn(
        "border-b border-(--border-soft) text-xs text-(--color-text-subtle)",
        className,
      )}
      {...props}
    />
  )
}

function TableBody({ className, ...props }: ComponentProps<"tbody">) {
  return (
    <tbody
      data-slot="table-body"
      className={cn("divide-y divide-(--border-soft)", className)}
      {...props}
    />
  )
}

function TableRow({ className, ...props }: ComponentProps<"tr">) {
  return (
    <tr
      data-slot="table-row"
      className={cn(
        "transition-colors hover:bg-(--bg-key)/45 [&:has(td[colspan])]:hover:bg-transparent",
        className,
      )}
      {...props}
    />
  )
}

function TableTh({ className, ...props }: ComponentProps<"th">) {
  return (
    <th
      data-slot="table-th"
      className={cn("px-4 py-2.5 font-medium whitespace-nowrap", className)}
      {...props}
    />
  )
}

function TableTd({ className, ...props }: ComponentProps<"td">) {
  return (
    <td
      data-slot="table-td"
      className={cn("px-4 py-3 align-middle", className)}
      {...props}
    />
  )
}

export { Table, TableBody, TableHead, TableRow, TableTd, TableTh, TableWrap }
