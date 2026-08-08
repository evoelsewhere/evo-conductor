import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"
import { useState } from "react"
import { Copy, Plus, Trash2 } from "lucide-react"

import { api, type SecretScope } from "@/shared/api/client"
import { Button } from "@/shared/ui/button"
import { Input } from "@/shared/ui/input"
import { Badge } from "@/shared/ui/badge"
import { PageFrame } from "@/shared/components/page-frame"

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
      subtitle="Generate tokens for EvoFlux to subscribe to this Conductor (Codex/Copilot-style machine auth)."
      action={
        <div className="flex items-center gap-2">
          <Input
            value={name}
            onChange={(e) => setName(e.target.value)}
            className="w-48"
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
        </div>
      }
    >
      {createdToken && (
        <div className="mb-4 rounded-lg border border-(--accent-blue)/30 bg-(--accent-blue-soft)/40 px-4 py-3">
          <div className="mb-1 text-xs font-medium text-(--accent-blue-text)">
            Copy this token now — it won’t be shown again
          </div>
          <div className="flex items-center gap-2">
            <code className="flex-1 truncate rounded-md bg-(--bg-page) px-2 py-1 font-mono text-xs">
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
        <p className="text-sm text-(--color-text-muted)">Loading…</p>
      ) : (
        <div className="overflow-hidden rounded-lg border border-(--border-card) bg-(--bg-card)">
          <table className="w-full text-left text-sm">
            <thead className="border-b border-(--border-soft) text-xs text-(--color-text-subtle)">
              <tr>
                <th className="px-4 py-2.5 font-medium">Name</th>
                <th className="px-4 py-2.5 font-medium">Prefix</th>
                <th className="px-4 py-2.5 font-medium">Scopes</th>
                <th className="px-4 py-2.5 font-medium">Status</th>
                <th className="px-4 py-2.5 font-medium" />
              </tr>
            </thead>
            <tbody className="divide-y divide-(--border-soft)">
              {data.map((s) => (
                <tr key={s.id}>
                  <td className="px-4 py-3 font-medium">{s.name}</td>
                  <td className="px-4 py-3 font-mono text-xs text-(--color-text-muted)">
                    evc_{s.prefix}_…
                  </td>
                  <td className="px-4 py-3">
                    <div className="flex flex-wrap gap-1">
                      {s.scopes.map((scope) => (
                        <Badge key={scope}>{scope}</Badge>
                      ))}
                    </div>
                  </td>
                  <td className="px-4 py-3 text-xs">
                    {s.revoked_at ? (
                      <span className="text-(--color-error)">revoked</span>
                    ) : (
                      <span className="text-(--color-success)">active</span>
                    )}
                  </td>
                  <td className="px-4 py-3 text-right">
                    {!s.revoked_at && (
                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={() => revoke.mutate(s.id)}
                      >
                        <Trash2 className="size-3.5" />
                      </Button>
                    )}
                  </td>
                </tr>
              ))}
              {data.length === 0 && (
                <tr>
                  <td
                    colSpan={5}
                    className="px-4 py-8 text-center text-sm text-(--color-text-subtle)"
                  >
                    No secrets yet — create one to connect EvoFlux.
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
      )}
    </PageFrame>
  )
}
