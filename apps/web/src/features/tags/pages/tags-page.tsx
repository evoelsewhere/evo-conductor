import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"
import { Pencil, Plus, Tags, Trash2 } from "lucide-react"
import { useState } from "react"

import { api, type Tag } from "@/shared/api/client"
import { PageFrame } from "@/shared/components/page-frame"
import { useAuthStore } from "@/shared/stores/auth"
import { Button } from "@/shared/ui/button"
import { Card, CardHeader, CardList, CardTitle } from "@/shared/ui/card"
import { EmptyState, ErrorState } from "@/shared/ui/empty-state"
import { Dialog } from "@/shared/ui/dialog"
import { Input } from "@/shared/ui/input"
import { Label } from "@/shared/ui/label"
import { SkeletonRows } from "@/shared/ui/skeleton"

export function TagsPage() {
  const user = useAuthStore((s) => s.user)
  const canManage =
    user?.primary_role === "admin" || user?.primary_role === "contribute"
  const qc = useQueryClient()
  const { data = [], isLoading, error } = useQuery({
    queryKey: ["tags"],
    queryFn: () => api.tags(),
  })
  const [editor, setEditor] = useState<Tag | "new" | null>(null)

  const remove = useMutation({
    mutationFn: (id: string) => api.deleteTag(id),
    onSuccess: () => void qc.invalidateQueries({ queryKey: ["tags"] }),
  })

  return (
    <PageFrame
      title="Tags"
      subtitle="Shared taxonomy for organizing teams, resources, policies, and future project entities."
      action={
        canManage ? (
          <Button variant="gradient" onClick={() => setEditor("new")}>
            <Plus className="size-3.5" />
            New tag
          </Button>
        ) : undefined
      }
    >
      {error && (
        <ErrorState
          className="mb-4"
          message={error instanceof Error ? error.message : "Failed to load"}
        />
      )}

      <Card>
        <CardHeader>
          <div>
            <CardTitle>Tag catalog</CardTitle>
            <p className="mt-1 text-xs text-(--color-text-muted)">
              Reusable labels available across the project.
            </p>
          </div>
        </CardHeader>

        {isLoading ? (
          <SkeletonRows rows={3} />
        ) : data.length === 0 ? (
          <div className="p-4">
            <EmptyState
              icon={Tags}
              title="No tags yet"
              description="Create reusable labels such as platform, frontend, critical, or squad-a."
              className="border-0 bg-transparent py-8"
            />
          </div>
        ) : (
          <CardList>
            {data.map((tag) => (
              <div key={tag.id} className="flex items-center gap-3 px-4 py-3">
                <span
                  className="size-2.5 shrink-0 rounded-full"
                  style={{ background: tag.color ?? "var(--color-accent)" }}
                />
                <div className="min-w-0 flex-1">
                  <div className="text-sm font-medium">{tag.name}</div>
                  <div className="font-mono text-[0.7rem] text-(--color-text-subtle)">
                    {tag.slug}
                  </div>
                </div>
                {canManage && (
                  <div className="flex items-center gap-1">
                    <Button
                      size="sm"
                      variant="ghost"
                      aria-label={`Edit ${tag.name}`}
                      onClick={() => setEditor(tag)}
                    >
                      <Pencil className="size-3.5" />
                    </Button>
                    <Button
                      size="sm"
                      variant="ghost"
                      aria-label={`Delete ${tag.name}`}
                      onClick={() => remove.mutate(tag.id)}
                    >
                      <Trash2 className="size-3.5" />
                    </Button>
                  </div>
                )}
              </div>
            ))}
          </CardList>
        )}
      </Card>
      {editor && (
        <TagDialog
          tag={editor}
          onClose={() => setEditor(null)}
          onSaved={() => {
            setEditor(null)
            void qc.invalidateQueries({ queryKey: ["tags"] })
          }}
        />
      )}
    </PageFrame>
  )
}

function TagDialog({
  tag,
  onClose,
  onSaved,
}: {
  tag: Tag | "new"
  onClose: () => void
  onSaved: () => void
}) {
  const existing = tag && tag !== "new" ? tag : null
  const [slug, setSlug] = useState(existing?.slug ?? "")
  const [name, setName] = useState(existing?.name ?? "")
  const [description, setDescription] = useState(existing?.description ?? "")
  const [color, setColor] = useState(existing?.color ?? "#667eea")
  const [error, setError] = useState<string | null>(null)

  const save = useMutation({
    mutationFn: () =>
      existing
        ? api.updateTag(existing.id, { name, description, color })
        : api.createTag({ slug, name, description, color }),
    onSuccess: onSaved,
    onError: (e) => setError(e instanceof Error ? e.message : "Save failed"),
  })

  return (
    <Dialog
      open
      title={existing ? "Edit tag" : "Create tag"}
      description="Tags are shared labels and are not limited to members."
      onClose={onClose}
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            Cancel
          </Button>
          <Button
            variant="gradient"
            disabled={!slug.trim() || !name.trim() || save.isPending}
            onClick={() => save.mutate()}
          >
            {existing ? "Save changes" : "Create tag"}
          </Button>
        </>
      }
    >
      <div className="space-y-4">
        <Field label="Name">
          <Input
            value={name}
            placeholder="Platform"
            autoFocus
            onChange={(e) => setName(e.target.value)}
          />
        </Field>
        <Field label="Slug">
          <Input
            value={slug}
            placeholder="platform"
            disabled={Boolean(existing)}
            onChange={(e) => setSlug(e.target.value)}
          />
        </Field>
        <Field label="Description">
          <Input
            value={description}
            placeholder="Used for platform-owned entities"
            onChange={(e) => setDescription(e.target.value)}
          />
        </Field>
        <Field label="Color">
          <div className="flex items-center gap-2">
            <input
              type="color"
              value={color}
              aria-label="Tag color"
              className="size-9 rounded-md border border-(--color-border) bg-transparent p-1"
              onChange={(e) => setColor(e.target.value)}
            />
            <Input value={color} onChange={(e) => setColor(e.target.value)} />
          </div>
        </Field>
        {error && <ErrorState message={error} />}
      </div>
    </Dialog>
  )
}

function Field({
  label,
  children,
}: {
  label: string
  children: React.ReactNode
}) {
  return (
    <div className="space-y-1.5">
      <Label>{label}</Label>
      {children}
    </div>
  )
}
