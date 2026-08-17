import { useQuery } from "@tanstack/react-query"
import { Boxes, KeyRound, Radio, Users } from "lucide-react"

import { api } from "@/shared/api/client"
import { PageFrame } from "@/shared/components/page-frame"
import { StatCard, StatCardSkeleton } from "@/shared/components/stat-card"
import { Badge } from "@/shared/ui/badge"
import {
  Card,
  CardFooter,
  CardHeader,
  CardList,
  CardTitle,
} from "@/shared/ui/card"
import { ErrorState } from "@/shared/ui/empty-state"

export function OverviewPage() {
  const { data, isLoading, error } = useQuery({
    queryKey: ["dashboard"],
    queryFn: () => api.dashboard(),
  })

  if (isLoading) {
    return (
      <PageFrame title="Overview" subtitle="Loading project metrics…">
        <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
          {Array.from({ length: 4 }, (_, i) => (
            <StatCardSkeleton key={i} />
          ))}
        </div>
      </PageFrame>
    )
  }

  if (error || !data) {
    return (
      <PageFrame title="Overview">
        <ErrorState
          message={
            error instanceof Error ? error.message : "Failed to load dashboard"
          }
        />
      </PageFrame>
    )
  }

  const resourceTotal =
    data.resources.agents +
    data.resources.skills +
    data.resources.plugins +
    data.resources.workflows

  return (
    <PageFrame
      title={data.project_name}
      subtitle="Central monitoring and resource control for every EvoFlux member."
    >
      <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
        <StatCard
          label="Members"
          value={data.members_total}
          hint={`${data.members_online} online via EvoFlux`}
          icon={Users}
          tone="accent"
        />
        <StatCard
          label="Active secrets"
          value={data.secrets_active}
          hint="EvoFlux connection tokens"
          icon={KeyRound}
        />
        <StatCard
          label="Shared resources"
          value={resourceTotal}
          hint={`${data.resources.agents} agents · ${data.resources.skills} skills · ${data.resources.plugins} plugins`}
          icon={Boxes}
          tone="success"
        />
        <StatCard
          label="SSO"
          value={data.sso_enabled ? "On" : "Off"}
          hint={
            data.sso_enabled ? "Org identity enabled" : "Password login only"
          }
          icon={Radio}
          tone={data.sso_enabled ? "success" : "warning"}
        />
      </div>

      <Card className="mt-6">
        <CardHeader>
          <CardTitle>How members connect</CardTitle>
        </CardHeader>
        <CardList>
          <Row
            title="Create a connection secret"
            body="Each member generates an evc_… token under Secrets, then pastes it into EvoFlux → Conductor subscribe."
          />
          <Row
            title="Subscribe Agents / Skills / Plugins"
            body="EvoFlux pulls shared catalogs from /api/v1/subscribe/resources and reports inventory heartbeats."
          />
          <Row
            title="Monitor performance"
            body="Admins can investigate member detail; Contributors receive privacy-safe project and owned-resource aggregates."
          />
        </CardList>
        <CardFooter>
          <Badge>admin</Badge>
          <Badge>contribute</Badge>
          <Badge>user + sub-roles</Badge>
        </CardFooter>
      </Card>
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
