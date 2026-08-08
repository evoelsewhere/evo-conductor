import { useQuery } from "@tanstack/react-query"
import { Boxes } from "lucide-react"

import { api } from "@/shared/api/client"
import { PageFrame } from "@/shared/components/page-frame"
import { Badge } from "@/shared/ui/badge"
import { EmptyState } from "@/shared/ui/empty-state"
import { SkeletonRows } from "@/shared/ui/skeleton"
import {
  Table,
  TableBody,
  TableHead,
  TableRow,
  TableTd,
  TableTh,
  TableWrap,
} from "@/shared/ui/table"

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
        <TableWrap>
          <SkeletonRows rows={4} />
        </TableWrap>
      ) : data.length === 0 ? (
        <EmptyState
          icon={Boxes}
          title="No shared resources yet"
          description="When EvoFlux instances subscribe and sync inventory, catalogs will appear here. Contribute role can also publish shared packages."
        />
      ) : (
        <TableWrap>
          <Table>
            <TableHead>
              <tr>
                <TableTh>Name</TableTh>
                <TableTh>Kind</TableTh>
                <TableTh>Version</TableTh>
                <TableTh>Visibility</TableTh>
              </tr>
            </TableHead>
            <TableBody>
              {data.map((r) => (
                <TableRow key={r.id}>
                  <TableTd>
                    <div className="font-medium">{r.name}</div>
                    <div className="font-mono text-[0.7rem] text-(--color-text-subtle)">
                      {r.slug}
                    </div>
                  </TableTd>
                  <TableTd>
                    <Badge className="capitalize">{r.kind}</Badge>
                  </TableTd>
                  <TableTd className="text-(--color-text-muted)">
                    {r.version}
                  </TableTd>
                  <TableTd className="capitalize text-(--color-text-muted)">
                    {r.visibility}
                  </TableTd>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </TableWrap>
      )}
    </PageFrame>
  )
}
