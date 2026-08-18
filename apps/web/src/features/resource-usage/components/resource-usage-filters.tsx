import { RotateCcw, Search, SlidersHorizontal } from "lucide-react"
import { useState } from "react"

import type {
  ClientInstallationSummary,
  ManagedResource,
  MemberListItem,
  ResourceVersion,
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
import { ErrorState } from "@/shared/ui/empty-state"
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
  membersLoading = false,
  membersError,
  installations,
  installationsLoading = false,
  installationsError,
  resources,
  resourcesLoading = false,
  resourcesError,
  versions,
  versionsLoading = false,
  versionsError,
  lockedKind,
  allowMemberDetail,
  onChange,
}: {
  value: ResourceUsageFilterState
  members: MemberListItem[]
  membersLoading?: boolean
  membersError?: string
  installations: ClientInstallationSummary[]
  installationsLoading?: boolean
  installationsError?: string
  resources: ManagedResource[]
  resourcesLoading?: boolean
  resourcesError?: string
  versions: ResourceVersion[]
  versionsLoading?: boolean
  versionsError?: string
  lockedKind?: Extract<ResourceKind, "plugin" | "skill" | "agent">
  allowMemberDetail: boolean
  onChange: (value: ResourceUsageFilterState) => void
}) {
  const [expanded, setExpanded] = useState(false)
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
  const activeCount = Object.entries(value).filter(([key, item]) => {
    if (key === "resourceKind" && lockedKind) return false
    if (key === "provider" || key === "model" || key === "toolName") return Boolean(item)
    return item !== RESOURCE_USAGE_ALL_FILTER
  }).length

  return (
    <div className="rounded-xl border border-(--border-card) bg-(--bg-card)">
      <div className="flex flex-col gap-2 p-3 lg:flex-row lg:items-center">
        {allowMemberDetail && (
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
              {
                value: RESOURCE_USAGE_ALL_FILTER,
                label: membersLoading
                  ? "Loading members…"
                  : membersError
                    ? "Members unavailable"
                    : "All members",
              },
              ...members.map((member) => ({ value: member.id, label: member.display_name })),
            ]}
            disabled={membersLoading || Boolean(membersError)}
            aria-busy={membersLoading}
            aria-label="Filter by member"
            className="lg:w-48"
          />
        )}
        {!lockedKind && (
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
            className="lg:w-44"
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
            {
              value: RESOURCE_USAGE_ALL_FILTER,
              label: resourcesLoading
                ? "Loading resources…"
                : resourcesError
                  ? "Resources unavailable"
                  : "All resources",
            },
            ...visibleResources.map((resource) => ({ value: resource.id, label: resource.name })),
          ]}
          disabled={resourcesLoading || Boolean(resourcesError)}
          aria-busy={resourcesLoading}
          aria-label="Filter by resource"
          className="lg:min-w-48 lg:flex-1"
        />
        <Select value={value.status} onValueChange={(next) => set("status", next)} options={[...RESOURCE_USAGE_STATUS_OPTIONS]} aria-label="Filter by outcome" className="lg:w-40" />
        <Button
          variant={expanded || activeCount > 0 ? "secondary" : "outline"}
          onClick={() => setExpanded((open) => !open)}
          aria-expanded={expanded}
        >
          <SlidersHorizontal className="size-3.5" />
          More filters
          {activeCount > 0 && (
            <span className="rounded-full bg-(--color-accent-soft) px-1.5 text-[0.65rem] text-(--color-accent)">
              {activeCount}
            </span>
          )}
        </Button>
      </div>

      {expanded && (
        <div className="border-t border-(--border-soft) p-3">
          <div className="mb-2 flex items-center justify-between gap-3">
            <div>
              <div className="text-xs font-medium">Advanced dimensions</div>
              <p className="mt-0.5 text-[0.68rem] text-(--color-text-subtle)">
                Narrow by delivery, version, role, relation, provider, model or tool.
              </p>
            </div>
            <Button
              variant="ghost"
              size="sm"
              disabled={!active}
              onClick={() => onChange({ ...EMPTY_RESOURCE_USAGE_FILTERS, resourceKind: lockedKind ?? RESOURCE_USAGE_ALL_FILTER })}
            >
              <RotateCcw className="size-3.5" /> Clear all
            </Button>
          </div>
          <div className="grid gap-2 sm:grid-cols-2 xl:grid-cols-4">
            {allowMemberDetail && (
              <Select
                value={value.installationId}
                onValueChange={(next) => set("installationId", next)}
                  options={[
                    {
                      value: RESOURCE_USAGE_ALL_FILTER,
                      label: installationsLoading
                        ? "Loading installations…"
                        : installationsError
                          ? "Installations unavailable"
                        : "All installations",
                    },
                  ...installations.map((installation) => ({
                    value: installation.id,
                    label: `${installation.display_name} · ${installation.platform}`,
                  })),
                ]}
                disabled={
                  value.memberId === RESOURCE_USAGE_ALL_FILTER ||
                  installationsLoading ||
                  Boolean(installationsError)
                }
                aria-busy={installationsLoading}
                aria-label="Filter by EvoFlux installation"
              />
            )}
            <Select value={value.primaryRole} onValueChange={(next) => set("primaryRole", next)} options={[...RESOURCE_USAGE_ROLE_OPTIONS]} aria-label="Filter by role" />
            <Select
              value={value.versionId}
              onValueChange={(next) => set("versionId", next)}
              options={[
                {
                  value: RESOURCE_USAGE_ALL_FILTER,
                  label: versionsLoading
                    ? "Loading versions…"
                    : versionsError
                      ? "Versions unavailable"
                      : "All versions",
                },
                ...versions.map((version) => ({ value: version.id, label: `v${version.version} · ${version.status}` })),
              ]}
              disabled={
                value.resourceId === RESOURCE_USAGE_ALL_FILTER ||
                versionsLoading ||
                Boolean(versionsError)
              }
              aria-busy={versionsLoading}
              aria-label="Filter by resource version"
            />
            <Select value={value.relation} onValueChange={(next) => set("relation", next)} options={[...RESOURCE_USAGE_RELATION_OPTIONS]} aria-label="Filter by attribution relation" />
            <SearchField value={value.provider} onChange={(next) => set("provider", next)} placeholder="Provider" ariaLabel="Filter by provider" />
            <SearchField value={value.model} onChange={(next) => set("model", next)} placeholder="Model" ariaLabel="Filter by model" />
            <SearchField value={value.toolName} onChange={(next) => set("toolName", next)} placeholder="Tool name" ariaLabel="Filter by tool name" />
            {lockedKind && (
              <div className="flex h-9 items-center rounded-md border border-(--border-soft) bg-(--bg-key) px-3 text-xs font-medium capitalize text-(--color-text-muted)">
                Locked to {lockedKind}s
              </div>
            )}
          </div>
        </div>
      )}
      {[membersError, installationsError, resourcesError, versionsError].some(Boolean) && (
        <ErrorState
          className="m-3 mt-0"
          message={[
            membersError && `Members: ${membersError}`,
            installationsError && `Installations: ${installationsError}`,
            resourcesError && `Resources: ${resourcesError}`,
            versionsError && `Versions: ${versionsError}`,
          ]
            .filter(Boolean)
            .join(" ")}
        />
      )}
    </div>
  )
}

function SearchField({
  value,
  onChange,
  placeholder,
  ariaLabel,
}: {
  value: string
  onChange: (value: string) => void
  placeholder: string
  ariaLabel: string
}) {
  return (
    <label className="relative">
      <Search className="pointer-events-none absolute top-1/2 left-3 size-3.5 -translate-y-1/2 text-(--color-text-subtle)" />
      <Input className="pl-8" value={value} onChange={(event) => onChange(event.target.value)} placeholder={placeholder} aria-label={ariaLabel} />
    </label>
  )
}
