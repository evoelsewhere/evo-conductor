import { useQuery } from "@tanstack/react-query"

import { api } from "@/shared/api/client"
import { Badge } from "@/shared/ui/badge"
import { PageFrame } from "@/shared/components/page-frame"

export function ResourcesPage() {
  const { data = [], isLoading } = useQuery({
    queryKey: ["resources"],
    queryFn: () => api.resources(),
  })

  return (
    <PageFrame
      title="Resources"
      subtitle="Shared agents, skills, MCP servers and workflows subscribed by EvoFlux members."
    >
      {isLoading ? (
        <p className="text-sm text-(--color-text-muted)">Loading…</p>
      ) : data.length === 0 ? (
        <div className="rounded-lg border border-dashed border-(--color-border) bg-(--bg-card)/50 px-6 py-12 text-center">
          <p className="text-sm font-medium">No shared resources yet</p>
          <p className="mt-1 text-xs text-(--color-text-muted)">
            When EvoFlux instances subscribe and sync inventory, catalogs will
            appear here. Contribute role can also publish shared packages.
          </p>
        </div>
      ) : (
        <div className="overflow-hidden rounded-lg border border-(--border-card) bg-(--bg-card)">
          <table className="w-full text-left text-sm">
            <thead className="border-b border-(--border-soft) text-xs text-(--color-text-subtle)">
              <tr>
                <th className="px-4 py-2.5 font-medium">Name</th>
                <th className="px-4 py-2.5 font-medium">Kind</th>
                <th className="px-4 py-2.5 font-medium">Version</th>
                <th className="px-4 py-2.5 font-medium">Visibility</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-(--border-soft)">
              {data.map((r) => (
                <tr key={r.id}>
                  <td className="px-4 py-3">
                    <div className="font-medium">{r.name}</div>
                    <div className="font-mono text-[0.7rem] text-(--color-text-subtle)">
                      {r.slug}
                    </div>
                  </td>
                  <td className="px-4 py-3">
                    <Badge className="capitalize">{r.kind}</Badge>
                  </td>
                  <td className="px-4 py-3 text-(--color-text-muted)">{r.version}</td>
                  <td className="px-4 py-3 capitalize text-(--color-text-muted)">
                    {r.visibility}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </PageFrame>
  )
}
