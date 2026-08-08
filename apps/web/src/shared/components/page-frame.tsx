import { cn } from "@/shared/lib/utils"

export function PageFrame({
  title,
  subtitle,
  children,
  action,
  className,
}: {
  title: string
  subtitle?: string
  children: React.ReactNode
  action?: React.ReactNode
  className?: string
}) {
  return (
    <div
      className={cn(
        "mx-auto w-full max-w-6xl px-4 py-5 sm:px-6 sm:py-6 md:px-8",
        className,
      )}
    >
      <div className="mb-5 flex flex-col gap-3 sm:mb-6 sm:flex-row sm:items-start sm:justify-between">
        <div className="min-w-0">
          <h1 className="text-xl font-semibold tracking-tight text-(--color-text)">
            {title}
          </h1>
          {subtitle && (
            <p className="mt-1 max-w-2xl text-sm text-(--color-text-muted)">
              {subtitle}
            </p>
          )}
        </div>
        {action && (
          <div className="flex shrink-0 flex-wrap items-center gap-2">{action}</div>
        )}
      </div>
      {children}
    </div>
  )
}
