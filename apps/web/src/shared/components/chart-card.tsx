import type { ComponentProps, ReactNode } from "react"

import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/shared/ui/card"
import { cn } from "@/shared/lib/utils"

export function ChartCard({
  title,
  description,
  action,
  children,
  className,
  contentClassName,
  ...props
}: Omit<ComponentProps<typeof Card>, "title"> & {
  title: ReactNode
  description?: ReactNode
  action?: ReactNode
  contentClassName?: string
}) {
  return (
    <Card className={className} {...props}>
      <CardHeader>
        <div>
          <CardTitle>{title}</CardTitle>
          {description && <CardDescription className="mt-0.5">{description}</CardDescription>}
        </div>
        {action}
      </CardHeader>
      <CardContent className={cn("min-w-0", contentClassName)}>{children}</CardContent>
    </Card>
  )
}
