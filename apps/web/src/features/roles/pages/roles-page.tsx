import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"
import { useState } from "react"
import { Plus } from "lucide-react"

import { api } from "@/shared/api/client"
import { Button } from "@/shared/ui/button"
import { Input } from "@/shared/ui/input"
import { useAuthStore } from "@/shared/stores/auth"
import { PageFrame } from "@/shared/components/page-frame"

export function RolesPage() {
  const user = useAuthStore((s) => s.user)
  const qc = useQueryClient()
  const { data = [], isLoading } = useQuery({
    queryKey: ["sub-roles"],
    queryFn: () => api.subRoles(),
  })
  const [slug, setSlug] = useState("")
  const [name, setName] = useState("")

  const create = useMutation({
    mutationFn: () => api.createSubRole({ slug, name }),
    onSuccess: () => {
      setSlug("")
      setName("")
      void qc.invalidateQueries({ queryKey: ["sub-roles"] })
    },
  })

  const canManage = user?.primary_role === "admin"

  return (
    <PageFrame
      title="Roles"
      subtitle="Primary roles are fixed. Sub-roles describe project membership (dev, ba, tester, …)."
    >
      <div className="mb-6 grid gap-3 md:grid-cols-3">
        {[
          {
            role: "admin",
            desc: "Setup, SSO, members, sub-roles, resource policy, full telemetry.",
          },
          {
            role: "contribute",
            desc: "Publish shared agents/skills/MCP and view team monitoring.",
          },
          {
            role: "user",
            desc: "Consume shared catalogs, create personal secrets, report usage.",
          },
        ].map((item) => (
          <div
            key={item.role}
            className="rounded-lg border border-(--border-card) bg-(--bg-card) p-4"
          >
            <div className="text-sm font-semibold capitalize">{item.role}</div>
            <p className="mt-1 text-xs text-(--color-text-muted)">{item.desc}</p>
          </div>
        ))}
      </div>

      <div className="rounded-lg border border-(--border-card) bg-(--bg-card)">
        <div className="flex items-center justify-between border-b border-(--border-soft) px-4 py-3">
          <h2 className="text-sm font-medium">Sub-roles</h2>
          {canManage && (
            <div className="flex items-center gap-2">
              <Input
                className="w-28"
                placeholder="slug"
                value={slug}
                onChange={(e) => setSlug(e.target.value)}
              />
              <Input
                className="w-36"
                placeholder="name"
                value={name}
                onChange={(e) => setName(e.target.value)}
              />
              <Button
                size="sm"
                variant="secondary"
                disabled={!slug.trim() || !name.trim() || create.isPending}
                onClick={() => create.mutate()}
              >
                <Plus className="size-3.5" />
                Add
              </Button>
            </div>
          )}
        </div>
        {isLoading ? (
          <div className="px-4 py-6 text-sm text-(--color-text-muted)">Loading…</div>
        ) : (
          <div className="divide-y divide-(--border-soft)">
            {data.map((role) => (
              <div key={role.id} className="flex items-center gap-3 px-4 py-3">
                <span
                  className="size-2.5 rounded-full"
                  style={{ background: role.color ?? "var(--color-accent)" }}
                />
                <div className="min-w-0 flex-1">
                  <div className="text-sm font-medium">{role.name}</div>
                  <div className="font-mono text-[0.7rem] text-(--color-text-subtle)">
                    {role.slug}
                  </div>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </PageFrame>
  )
}
