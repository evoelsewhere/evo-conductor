import { RotateCcw, Search } from "lucide-react"

import type {
  ClientInstallationSummary,
  ManagedResource,
  ResourceVersion,
  User,
} from "@/shared/api/client"
import type { ResourceKind } from "@/shared/constants/resource"
import {
  RESOURCE_USAGE_ALL_FILTER,
  RESOURCE_USAGE_KIND_OPTIONS,
  RESOURCE_USAGE_ROLE_OPTIONS,
  RESOURCE_USAGE_RELATION_OPTIONS,
  RESOURCE_USAGE_STATUS_OPTIONS,
} from "@/shared/constants/resource-usage"
import { Button } from "@/shared/ui/button"
import { Input } from "@/shared/ui/input"
import { Select } from "@/shared/ui/select"

export interface ResourceUsageFilterState {
  memberId: string
  installationId: string
  primaryRole: string
  resourceKind: string
  resourceId: string
  versionId: string
  status: string
  relation: string
  provider: string
  model: string
  toolName: string
}

export const EMPTY_RESOURCE_USAGE_FILTERS: ResourceUsageFilterState = {
  memberId: RESOURCE_USAGE_ALL_FILTER,
  installationId: RESOURCE_USAGE_ALL_FILTER,
  primaryRole: RESOURCE_USAGE_ALL_FILTER,
  resourceKind: RESOURCE_USAGE_ALL_FILTER,
  resourceId: RESOURCE_USAGE_ALL_FILTER,
  versionId: RESOURCE_USAGE_ALL_FILTER,
  status: RESOURCE_USAGE_ALL_FILTER,
  relation: RESOURCE_USAGE_ALL_FILTER,
  provider: "",
  model: "",
  toolName: "",
}

export function ResourceUsageFilters({
  value,
  members,
  installations,
  resources,
  versions,
  lockedKind,
  onChange,
}: {
  value: ResourceUsageFilterState
  members: User[]
  installations: ClientInstallationSummary[]
  resources: ManagedResource[]
  versions: ResourceVersion[]
  lockedKind?: Extract<ResourceKind, "plugin" | "skill" | "agent">
  onChange: (value: ResourceUsageFilterState) => void
}) {
  const set = (key: keyof ResourceUsageFilterState, next: string) =>
    onChange({ ...value, [key]: next })
  const visibleResources = resources.filter(
    (resource) =>
      value.resourceKind === RESOURCE_USAGE_ALL_FILTER ||
      resource.kind === value.resourceKind,
  )
  const active = Object.entries(value).some(
    ([key, item]) =>
      key === "resourceKind" && lockedKind
        ? false
        :
      key === "provider" || key === "model" || key === "toolName"
        ? Boolean(item)
        : item !== RESOURCE_USAGE_ALL_FILTER,
  )

  return (
    <div className="rounded-xl border border-(--border-card) bg-(--bg-card) p-3">
      <div className="grid gap-2 sm:grid-cols-2 xl:grid-cols-4">
        <Select
          value={value.memberId}
          onValueChange={(next) =>
            onChange({
              ...value,
              memberId: next,
              installationId: RESOURCE_USAGE_ALL_FILTER,
            })
          }
          options={[
            { value: RESOURCE_USAGE_ALL_FILTER, label: "All members" },
            ...members.map((member) => ({ value: member.id, label: member.display_name })),
          ]}
          aria-label="Filter by member"
        />
        <Select
          value={value.installationId}
          onValueChange={(next) => set("installationId", next)}
          options={[
            { value: RESOURCE_USAGE_ALL_FILTER, label: "All installations" },
            ...installations.map((installation) => ({
              value: installation.id,
              label: `${installation.display_name} · ${installation.platform}`,
            })),
          ]}
          disabled={value.memberId === RESOURCE_USAGE_ALL_FILTER}
          aria-label="Filter by EvoFlux installation"
        />
        <Select value={value.primaryRole} onValueChange={(next) => set("primaryRole", next)} options={[...RESOURCE_USAGE_ROLE_OPTIONS]} aria-label="Filter by role" />
        {lockedKind ? (
          <div className="flex h-9 items-center rounded-lg border border-(--border-soft) bg-(--bg-key) px-3 text-xs font-medium capitalize text-(--color-text-muted)">
            {lockedKind} scope
          </div>
        ) : (
          <Select
            value={value.resourceKind}
            onValueChange={(next) =>
              onChange({
                ...value,
                resourceKind: next,
                resourceId: RESOURCE_USAGE_ALL_FILTER,
                versionId: RESOURCE_USAGE_ALL_FILTER,
              })
            }
            options={[...RESOURCE_USAGE_KIND_OPTIONS]}
            aria-label="Filter by resource kind"
          />
        )}
        <Select
          value={value.resourceId}
          onValueChange={(next) =>
            onChange({
              ...value,
              resourceId: next,
              versionId: RESOURCE_USAGE_ALL_FILTER,
            })
          }
          options={[
            { value: RESOURCE_USAGE_ALL_FILTER, label: "All resources" },
            ...visibleResources.map((resource) => ({ value: resource.id, label: resource.name })),
          ]}
          aria-label="Filter by resource"
        />
        <Select
          value={value.versionId}
          onValueChange={(next) => set("versionId", next)}
          options={[
            { value: RESOURCE_USAGE_ALL_FILTER, label: "All versions" },
            ...versions.map((version) => ({ value: version.id, label: `v${version.version} · ${version.status}` })),
          ]}
          disabled={value.resourceId === RESOURCE_USAGE_ALL_FILTER}
          aria-label="Filter by resource version"
        />
        <Select value={value.status} onValueChange={(next) => set("status", next)} options={[...RESOURCE_USAGE_STATUS_OPTIONS]} aria-label="Filter by outcome" />
        <Select value={value.relation} onValueChange={(next) => set("relation", next)} options={[...RESOURCE_USAGE_RELATION_OPTIONS]} aria-label="Filter by attribution relation" />
        <label className="relative">
          <Search className="pointer-events-none absolute top-1/2 left-3 size-3.5 -translate-y-1/2 text-(--color-text-subtle)" />
          <Input className="pl-8" value={value.provider} onChange={(event) => set("provider", event.target.value)} placeholder="Provider" aria-label="Filter by provider" />
        </label>
        <label className="relative">
          <Search className="pointer-events-none absolute top-1/2 left-3 size-3.5 -translate-y-1/2 text-(--color-text-subtle)" />
          <Input className="pl-8" value={value.toolName} onChange={(event) => set("toolName", event.target.value)} placeholder="Tool name" aria-label="Filter by tool name" />
        </label>
        <label className="relative">
          <Search className="pointer-events-none absolute top-1/2 left-3 size-3.5 -translate-y-1/2 text-(--color-text-subtle)" />
          <Input className="pl-8" value={value.model} onChange={(event) => set("model", event.target.value)} placeholder="Model" aria-label="Filter by model" />
        </label>
        <Button variant="outline" disabled={!active} onClick={() => onChange({ ...EMPTY_RESOURCE_USAGE_FILTERS, resourceKind: lockedKind ?? RESOURCE_USAGE_ALL_FILTER })}>
          <RotateCcw className="size-3.5" />Clear filters
        </Button>
      </div>
    </div>
  )
}
