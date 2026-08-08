import { useQuery } from "@tanstack/react-query"

import { api } from "@/shared/api/client"
import { Badge } from "@/shared/ui/badge"
import { PageFrame } from "@/shared/components/page-frame"

export function MembersPage() {
  const { data = [], isLoading, error } = useQuery({
    queryKey: ["members"],
    queryFn: () => api.members(),
  })

  return (
    <PageFrame
      title="Members"
      subtitle="Primary roles: admin · contribute · user. Sub-roles (dev, ba, tester, …) are defined by admin."
    >
      {isLoading && <p className="text-sm text-(--color-text-muted)">Loading…</p>}
      {error && (
        <p className="text-sm text-(--color-error)">
          {error instanceof Error ? error.message : "Failed to load"}
        </p>
      )}
      <div className="overflow-hidden rounded-lg border border-(--border-card) bg-(--bg-card)">
        <table className="w-full text-left text-sm">
          <thead className="border-b border-(--border-soft) text-xs text-(--color-text-subtle)">
            <tr>
              <th className="px-4 py-2.5 font-medium">Member</th>
              <th className="px-4 py-2.5 font-medium">Primary role</th>
              <th className="px-4 py-2.5 font-medium">Status</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-(--border-soft)">
            {data.map((m) => (
              <tr key={m.id} className="hover:bg-(--bg-key)/50">
                <td className="px-4 py-3">
                  <div className="font-medium">{m.display_name}</div>
                  <div className="text-xs text-(--color-text-subtle)">{m.email}</div>
                </td>
                <td className="px-4 py-3">
                  <Badge className="capitalize">{m.primary_role}</Badge>
                </td>
                <td className="px-4 py-3 capitalize text-(--color-text-muted)">
                  {m.status}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </PageFrame>
  )
}
