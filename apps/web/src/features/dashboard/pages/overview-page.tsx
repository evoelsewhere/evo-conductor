import { useQuery } from "@tanstack/react-query"
import { Boxes, KeyRound, Radio, Users } from "lucide-react"

import { api } from "@/shared/api/client"
import { Badge } from "@/shared/ui/badge"
import { PageFrame } from "@/shared/components/page-frame"

export function OverviewPage() {
  const { data, isLoading, error } = useQuery({
    queryKey: ["dashboard"],
    queryFn: () => api.dashboard(),
  })

  if (isLoading) {
    return <PageFrame title="Overview">Loading…</PageFrame>
  }

  if (error || !data) {
    return (
      <PageFrame title="Overview">
        <p className="text-sm text-(--color-error)">
          {error instanceof Error ? error.message : "Failed to load dashboard"}
        </p>
      </PageFrame>
    )
  }

  const cards = [
    {
      label: "Members",
      value: data.members_total,
      hint: `${data.members_online} online via EvoFlux`,
      icon: Users,
    },
    {
      label: "Active secrets",
      value: data.secrets_active,
      hint: "EvoFlux connection tokens",
      icon: KeyRound,
    },
    {
      label: "Shared resources",
      value:
        data.resources.agents +
        data.resources.skills +
        data.resources.mcp +
        data.resources.workflows,
      hint: `${data.resources.agents} agents · ${data.resources.skills} skills · ${data.resources.mcp} mcp`,
      icon: Boxes,
    },
    {
      label: "SSO",
      value: data.sso_enabled ? "On" : "Off",
      hint: data.sso_enabled ? "Org identity enabled" : "Password login only",
      icon: Radio,
    },
  ]

  return (
    <PageFrame
      title={data.project_name}
      subtitle="Central monitoring and resource control for every EvoFlux member."
    >
      <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
        {cards.map((card) => (
          <div
            key={card.label}
            className="rounded-lg border border-(--border-card) bg-(--bg-card) p-4"
          >
            <div className="mb-3 flex items-center justify-between">
              <span className="text-xs text-(--color-text-muted)">{card.label}</span>
              <card.icon className="size-4 text-(--color-text-subtle)" strokeWidth={1.65} />
            </div>
            <div className="text-2xl font-semibold tracking-tight">{card.value}</div>
            <div className="mt-1 text-xs text-(--color-text-subtle)">{card.hint}</div>
          </div>
        ))}
      </div>

      <div className="mt-6 rounded-lg border border-(--border-card) bg-(--bg-card)">
        <div className="border-b border-(--border-soft) px-4 py-3">
          <h2 className="text-sm font-medium">How members connect</h2>
        </div>
        <div className="divide-y divide-(--border-soft)">
          <Row
            title="Create a connection secret"
            body="Each member generates an evc_… token under Secrets, then pastes it into EvoFlux → Conductor subscribe."
          />
          <Row
            title="Subscribe agents / skills / MCP"
            body="EvoFlux pulls shared catalogs from /api/v1/subscribe/resources and reports inventory heartbeats."
          />
          <Row
            title="Monitor performance"
            body="Admin and Contribute roles see per-member usage: tokens, tool calls, active agents."
          />
        </div>
        <div className="flex gap-2 px-4 py-3">
          <Badge>admin</Badge>
          <Badge>contribute</Badge>
          <Badge>user + sub-roles</Badge>
        </div>
      </div>
    </PageFrame>
  )
}

function Row({ title, body }: { title: string; body: string }) {
  return (
    <div className="px-4 py-3">
      <div className="text-sm font-medium text-(--color-text)">{title}</div>
      <div className="mt-0.5 text-xs text-(--color-text-muted)">{body}</div>
    </div>
  )
}
