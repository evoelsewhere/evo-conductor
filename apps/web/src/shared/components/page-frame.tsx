export function PageFrame({
  title,
  subtitle,
  children,
  action,
}: {
  title: string
  subtitle?: string
  children: React.ReactNode
  action?: React.ReactNode
}) {
  return (
    <div className="px-6 py-6 md:px-8">
      <div className="mb-6 flex items-start justify-between gap-4">
        <div>
          <h1 className="text-xl font-semibold tracking-tight">{title}</h1>
          {subtitle && (
            <p className="mt-1 text-sm text-(--color-text-muted)">{subtitle}</p>
          )}
        </div>
        {action}
      </div>
      {children}
    </div>
  )
}
