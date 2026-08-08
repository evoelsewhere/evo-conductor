import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"
import { useState } from "react"
import { Copy, KeyRound, Plus, Trash2 } from "lucide-react"

import { api, type SecretScope } from "@/shared/api/client"
import { PageFrame } from "@/shared/components/page-frame"
import { Badge } from "@/shared/ui/badge"
import { BadgeList } from "@/shared/ui/badge-list"
import { Button } from "@/shared/ui/button"
import { EmptyState } from "@/shared/ui/empty-state"
import { Input } from "@/shared/ui/input"
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

const defaultScopes: SecretScope[] = [
  "subscribe_resources",
  "report_telemetry",
  "sync_inventory",
]

export function SecretsPage() {
  const qc = useQueryClient()
  const { data = [], isLoading } = useQuery({
    queryKey: ["secrets"],
    queryFn: () => api.secrets(),
  })
  const [name, setName] = useState("EvoFlux laptop")
  const [createdToken, setCreatedToken] = useState<string | null>(null)

  const create = useMutation({
    mutationFn: () => api.createSecret({ name, scopes: defaultScopes }),
    onSuccess: (res) => {
      setCreatedToken(res.token)
      void qc.invalidateQueries({ queryKey: ["secrets"] })
    },
  })

  const revoke = useMutation({
    mutationFn: (id: string) => api.revokeSecret(id),
    onSuccess: () => void qc.invalidateQueries({ queryKey: ["secrets"] }),
  })

  return (
    <PageFrame
      title="Connection secrets"
      subtitle="Generate tokens for EvoFlux to subscribe to this Conductor."
      action={
        <>
          <Input
            value={name}
            onChange={(e) => setName(e.target.value)}
            className="w-full sm:w-48"
            placeholder="Secret name"
          />
          <Button
            variant="gradient"
            onClick={() => create.mutate()}
            disabled={create.isPending || !name.trim()}
          >
            <Plus className="size-3.5" />
            Create
          </Button>
        </>
      }
    >
      {createdToken && (
        <div className="mb-4 rounded-xl border border-(--accent-blue)/30 bg-(--accent-blue)/8 px-4 py-3">
          <div className="mb-1 text-xs font-medium text-(--accent-blue-text)">
            Copy this token now — it won’t be shown again
          </div>
          <div className="flex flex-col gap-2 sm:flex-row sm:items-center">
            <code className="min-w-0 flex-1 truncate rounded-md bg-(--bg-page) px-2 py-1.5 font-mono text-xs">
              {createdToken}
            </code>
            <Button
              variant="outline"
              size="sm"
              onClick={() => void navigator.clipboard.writeText(createdToken)}
            >
              <Copy className="size-3.5" />
              Copy
            </Button>
          </div>
        </div>
      )}

      {isLoading ? (
        <TableWrap>
          <SkeletonRows rows={4} />
        </TableWrap>
      ) : data.length === 0 ? (
        <EmptyState
          icon={KeyRound}
          title="No secrets yet"
          description="Create a connection token and paste it into EvoFlux to subscribe to this Conductor."
        />
      ) : (
        <TableWrap>
          <Table>
            <TableHead>
              <tr>
                <TableTh>Name</TableTh>
                <TableTh>Prefix</TableTh>
                <TableTh>Scopes</TableTh>
                <TableTh>Status</TableTh>
                <TableTh />
              </tr>
            </TableHead>
            <TableBody>
              {data.map((s) => (
                <TableRow key={s.id}>
                  <TableTd className="font-medium">{s.name}</TableTd>
                  <TableTd className="font-mono text-xs text-(--color-text-muted)">
                    evc_{s.prefix}_…
                  </TableTd>
                  <TableTd>
                    <BadgeList className="max-w-xs" max={3} items={s.scopes} />
                  </TableTd>
                  <TableTd className="text-xs">
                    {s.revoked_at ? (
                      <Badge tone="danger">revoked</Badge>
                    ) : (
                      <Badge tone="success">active</Badge>
                    )}
                  </TableTd>
                  <TableTd className="text-right">
                    {!s.revoked_at && (
                      <Button
                        variant="ghost"
                        size="sm"
                        aria-label={`Revoke ${s.name}`}
                        onClick={() => revoke.mutate(s.id)}
                      >
                        <Trash2 className="size-3.5" />
                      </Button>
                    )}
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
