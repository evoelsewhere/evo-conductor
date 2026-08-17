import { useMutation } from "@tanstack/react-query"
import { GitBranch, History, RotateCcw, TriangleAlert } from "lucide-react"
import { useState } from "react"

import {
  api,
  type DraftFileTree,
  type ManagedResource,
  type ResourceVersion,
} from "@/shared/api/client"
import {
  RELEASE_CHANNEL,
  RESOURCE_STATUS,
  RESOURCE_VERSION_REASON_MAX_LENGTH,
  RESOURCE_VERSION_STATUS,
} from "@/shared/constants/resource"
import { Badge } from "@/shared/ui/badge"
import { Button } from "@/shared/ui/button"
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/shared/ui/card"
import { Dialog } from "@/shared/ui/dialog"
import { EmptyState, ErrorState } from "@/shared/ui/empty-state"
import { Label } from "@/shared/ui/label"
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
import { Textarea } from "@/shared/ui/textarea"

interface ResourceVersionHistoryProps {
  resource: ManagedResource
  draftRevision: number
  loading: boolean
  versions?: ResourceVersion[]
  canManageLifecycle: boolean
  onVersionDeprecated: (version: ResourceVersion) => void
  onDraftRestored: (tree: DraftFileTree, version: ResourceVersion) => void
}

export function ResourceVersionHistory({
  resource,
  draftRevision,
  loading,
  versions,
  canManageLifecycle,
  onVersionDeprecated,
  onDraftRestored,
}: ResourceVersionHistoryProps) {
  const [deprecating, setDeprecating] = useState<ResourceVersion | null>(null)
  const [restoring, setRestoring] = useState<ResourceVersion | null>(null)
  const [reason, setReason] = useState("")
  const [deprecatedAcknowledged, setDeprecatedAcknowledged] = useState(false)

  const deprecate = useMutation({
    mutationFn: () => {
      if (!deprecating) throw new Error("Select a version to deprecate.")
      return api.deprecateResourceVersion(resource.id, deprecating.id, reason.trim())
    },
    onSuccess: (version) => {
      setDeprecating(null)
      setReason("")
      onVersionDeprecated(version)
    },
  })

  const restore = useMutation({
    mutationFn: async () => {
      if (!restoring) throw new Error("Select a version to restore.")
      const tree = await api.restoreResourceVersionToDraft(
        resource.id,
        restoring.id,
        draftRevision,
        restoring.status === RESOURCE_VERSION_STATUS.DEPRECATED && deprecatedAcknowledged,
      )
      return { tree, version: restoring }
    },
    onSuccess: ({ tree, version }) => {
      setRestoring(null)
      setDeprecatedAcknowledged(false)
      onDraftRestored(tree, version)
    },
  })

  if (loading) return <SkeletonRows rows={6} />
  if (!versions?.length) {
    return (
      <EmptyState
        icon={GitBranch}
        title="No immutable releases yet"
        description="Validate the Draft and release v0.1.0 to Beta or Published. Saving files does not allocate a version."
      />
    )
  }

  const archived = resource.status === RESOURCE_STATUS.ARCHIVED
  const deprecatedRestore = restoring?.status === RESOURCE_VERSION_STATUS.DEPRECATED

  return (
    <>
      <Card>
        <CardHeader>
          <div>
            <CardTitle>Immutable release history</CardTitle>
            <CardDescription>
              Restore copies source into the mutable Draft. Deprecation preserves files, integrity metadata and audit history.
            </CardDescription>
          </div>
        </CardHeader>
        <CardContent className="p-0">
          {(deprecate.error || restore.error) && (
            <ErrorState
              className="m-4"
              message={
                [deprecate.error, restore.error].find((error) => error instanceof Error)
                  ?.message ?? "Version lifecycle action failed"
              }
            />
          )}
          <TableWrap className="rounded-none border-0">
            <Table>
              <TableHead>
                <tr>
                  <TableTh>Version</TableTh>
                  <TableTh>Lifecycle</TableTh>
                  <TableTh>Integrity</TableTh>
                  <TableTh>Released</TableTh>
                  {canManageLifecycle && <TableTh className="text-right">Actions</TableTh>}
                </tr>
              </TableHead>
              <TableBody>
                {versions.map((version) => {
                  const deprecated = version.status === RESOURCE_VERSION_STATUS.DEPRECATED
                  const active = version.active_channel !== null
                  return (
                    <TableRow key={version.id}>
                      <TableTd>
                        <div className="font-mono font-medium">v{version.version}</div>
                        <div className="mt-1 text-[0.68rem] text-(--color-text-subtle)">
                          {formatBytes(version.content_size)}
                        </div>
                      </TableTd>
                      <TableTd>
                        <VersionLifecycleBadge version={version} />
                        {deprecated && version.deprecation_reason && (
                          <p className="mt-1 max-w-72 text-[0.68rem] leading-relaxed text-(--color-text-muted)">
                            {version.deprecation_reason}
                          </p>
                        )}
                      </TableTd>
                      <TableTd className="font-mono text-[0.68rem] text-(--color-text-muted)">
                        {version.content_sha256 ? `${version.content_sha256.slice(0, 12)}…` : "Legacy"}
                      </TableTd>
                      <TableTd className="text-xs text-(--color-text-muted)">
                        {new Date(version.created_at).toLocaleString()}
                        {version.deprecated_at && (
                          <div className="mt-1 text-(--color-warning)">
                            Deprecated {new Date(version.deprecated_at).toLocaleString()}
                          </div>
                        )}
                      </TableTd>
                      {canManageLifecycle && <TableTd>
                        <div className="flex justify-end gap-1.5">
                          <Button
                            variant="outline"
                            size="sm"
                            disabled={archived || version.status === RESOURCE_VERSION_STATUS.DRAFT}
                            onClick={() => {
                              restore.reset()
                              setDeprecatedAcknowledged(false)
                              setRestoring(version)
                            }}
                          >
                            <RotateCcw className="size-3.5" />
                            Restore
                          </Button>
                          {!deprecated && (
                            <Button
                              variant="destructive"
                              size="sm"
                              disabled={
                                archived || active || version.status === RESOURCE_VERSION_STATUS.DRAFT
                              }
                              title={
                                active
                                  ? "Release a replacement before deprecating the active channel version."
                                  : version.status === RESOURCE_VERSION_STATUS.DRAFT
                                    ? "Only immutable Beta or Published releases can be deprecated."
                                  : "Mark this historical version as deprecated."
                              }
                              onClick={() => {
                                deprecate.reset()
                                setReason("")
                                setDeprecating(version)
                              }}
                            >
                              <History className="size-3.5" />
                              Deprecate
                            </Button>
                          )}
                        </div>
                      </TableTd>}
                    </TableRow>
                  )
                })}
              </TableBody>
            </Table>
          </TableWrap>
        </CardContent>
      </Card>

      <Dialog
        open={canManageLifecycle && deprecating !== null}
        title={`Deprecate v${deprecating?.version ?? ""}?`}
        description="The immutable files and history remain available, but this version will no longer be a normal restore source."
        onClose={() => {
          if (deprecate.isPending) return
          setDeprecating(null)
          setReason("")
          deprecate.reset()
        }}
        footer={
          <>
            <Button variant="ghost" disabled={deprecate.isPending} onClick={() => setDeprecating(null)}>
              Cancel
            </Button>
            <Button
              variant="destructive"
              disabled={deprecate.isPending || !reason.trim()}
              onClick={() => deprecate.mutate()}
            >
              {deprecate.isPending ? "Deprecating…" : "Deprecate version"}
            </Button>
          </>
        }
      >
        <div className="space-y-4">
          <div className="flex gap-2 rounded-lg border border-(--color-warning)/30 bg-(--color-warning)/8 p-3 text-xs leading-relaxed text-(--color-text-muted)">
            <TriangleAlert className="mt-0.5 size-4 shrink-0 text-(--color-warning)" />
            This does not delete the version or change connected EvoFlux clients. Active channel versions must be replaced before deprecation.
          </div>
          {deprecate.error && (
            <ErrorState message={deprecate.error instanceof Error ? deprecate.error.message : "Deprecation failed"} />
          )}
          <div>
            <Label htmlFor="deprecation-reason">Reason</Label>
            <Textarea
              id="deprecation-reason"
              className="mt-1.5 min-h-24"
              maxLength={RESOURCE_VERSION_REASON_MAX_LENGTH}
              value={reason}
              onChange={(event) => setReason(event.target.value)}
              placeholder="Explain why this version should not be selected again."
            />
            <p className="mt-1 text-right text-[0.68rem] text-(--color-text-subtle)">
              {reason.length}/{RESOURCE_VERSION_REASON_MAX_LENGTH}
            </p>
          </div>
        </div>
      </Dialog>

      <Dialog
        open={canManageLifecycle && restoring !== null}
        title={`Restore v${restoring?.version ?? ""} to Draft?`}
        description="This replaces the current mutable Draft only. Active Beta or Published content stays unchanged until a new greater version is released."
        onClose={() => {
          if (restore.isPending) return
          setRestoring(null)
          setDeprecatedAcknowledged(false)
          restore.reset()
        }}
        footer={
          <>
            <Button variant="ghost" disabled={restore.isPending} onClick={() => setRestoring(null)}>
              Cancel
            </Button>
            <Button
              disabled={restore.isPending || (deprecatedRestore && !deprecatedAcknowledged)}
              onClick={() => restore.mutate()}
            >
              <RotateCcw className="size-3.5" />
              {restore.isPending ? "Restoring…" : "Restore to Draft"}
            </Button>
          </>
        }
      >
        <div className="space-y-4">
          {restore.error && (
            <ErrorState message={restore.error instanceof Error ? restore.error.message : "Restore failed"} />
          )}
          <div className="rounded-lg border border-(--border-card) bg-(--bg-key) p-3 text-xs leading-relaxed text-(--color-text-muted)">
            Current Draft revision {draftRevision} will be replaced. The restored content will receive a new higher semantic version only when you release it.
          </div>
          {deprecatedRestore && (
            <label className="flex cursor-pointer items-start gap-2 rounded-lg border border-(--color-warning)/35 bg-(--color-warning)/8 p-3 text-xs leading-relaxed">
              <input
                type="checkbox"
                className="mt-0.5 size-4 accent-(--color-warning)"
                checked={deprecatedAcknowledged}
                onChange={(event) => setDeprecatedAcknowledged(event.target.checked)}
              />
              <span>
                I understand that v{restoring?.version} was explicitly deprecated
                {restoring?.deprecation_reason ? `: ${restoring.deprecation_reason}` : "."}
              </span>
            </label>
          )}
        </div>
      </Dialog>
    </>
  )
}

function VersionLifecycleBadge({ version }: { version: ResourceVersion }) {
  if (version.status === RESOURCE_VERSION_STATUS.DEPRECATED) {
    return <Badge tone="danger">Deprecated</Badge>
  }
  if (version.active_channel === RELEASE_CHANNEL.PUBLISHED) {
    return <Badge tone="success">Active Published</Badge>
  }
  if (version.active_channel === RELEASE_CHANNEL.BETA) {
    return <Badge tone="warning">Active Beta</Badge>
  }
  return (
    <Badge tone="neutral" className="capitalize">
      Historical {version.release_channel ?? version.status}
    </Badge>
  )
}

function formatBytes(bytes: number) {
  if (!bytes) return "—"
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KiB`
  return `${(bytes / 1024 / 1024).toFixed(1)} MiB`
}
