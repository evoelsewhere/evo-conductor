import { Link, useRouterState } from "@tanstack/react-router"

import { RESOURCE_KIND_USAGE_PATHS } from "@/shared/constants/resource-monitoring"
import { RESOURCE_USAGE_PATHS, RESOURCE_USAGE_VIEW } from "@/shared/constants/resource-usage"
import type { ResourceKind } from "@/shared/constants/resource"
import { PERMISSION, mayRequest } from "@/shared/lib/authorization"
import { cn } from "@/shared/lib/utils"
import { useAuthStore } from "@/shared/stores/auth"

export function ResourceUsageNav({ kind }: { kind?: Extract<ResourceKind, "plugin" | "skill" | "agent"> }) {
  const pathname = useRouterState({ select: (state) => state.location.pathname })
  const can = useAuthStore((state) => state.can)
  const showIdentifyingActivity = mayRequest(can(PERMISSION.TELEMETRY_MEMBER_READ_ANY))
  const paths = kind ? RESOURCE_KIND_USAGE_PATHS[kind] : RESOURCE_USAGE_PATHS
  const items = [
    {
      view: RESOURCE_USAGE_VIEW.OVERVIEW,
      label: kind ? "Catalog" : "Monitor",
      to: paths.overview,
    },
    { view: RESOURCE_USAGE_VIEW.ACTIVITY, label: "Activity log", to: paths.activity },
    { view: RESOURCE_USAGE_VIEW.USAGE, label: "Analytics Studio", to: paths.usage },
  ] as const

  return (
    <nav
      className="mb-5 flex gap-1 overflow-x-auto border-b border-(--border-soft)"
      aria-label="Resource usage analytics"
    >
      {items
        .filter(
          (item) =>
            item.view !== RESOURCE_USAGE_VIEW.ACTIVITY || showIdentifyingActivity,
        )
        .map((item) => {
        const active =
          item.view === RESOURCE_USAGE_VIEW.OVERVIEW
            ? pathname === item.to
            : pathname === item.to || pathname.startsWith(`${item.to}/`)
        return (
          <Link
            key={item.view}
            to={item.to}
            search
            className={cn(
              "border-b-2 px-3 py-2 text-sm whitespace-nowrap transition-colors",
              active
                ? "border-(--color-accent) font-medium text-(--color-text)"
                : "border-transparent text-(--color-text-muted) hover:text-(--color-text)",
            )}
          >
            {item.label}
          </Link>
        )
      })}
    </nav>
  )
}
