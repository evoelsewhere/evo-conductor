import { Link, useRouterState } from "@tanstack/react-router"

import { MEMBER_ANALYTICS_NAV_ITEMS } from "@/shared/constants/member"
import { cn } from "@/shared/lib/utils"

export function MemberNav({ userId }: { userId: string }) {
  const pathname = useRouterState({ select: (state) => state.location.pathname })
  return (
    <nav
      className="mb-5 flex gap-1 overflow-x-auto border-b border-(--border-soft)"
      aria-label="Member analytics"
    >
      {MEMBER_ANALYTICS_NAV_ITEMS.map((item) => {
        const to = `/app/members/${userId}${item.suffix}`
        const active =
          item.suffix === ""
            ? pathname === to
            : pathname === to || pathname.startsWith(`${to}/`)
        return (
          <Link
            key={item.suffix}
            to={to}
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
