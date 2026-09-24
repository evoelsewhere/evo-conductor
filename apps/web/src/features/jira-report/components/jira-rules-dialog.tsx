import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"
import { useEffect, useState } from "react"

import {
  api,
  type ProjectPrefixRule,
  type TaskTypeRule,
} from "@/shared/api/client"
import { Button } from "@/shared/ui/button"
import { Dialog } from "@/shared/ui/dialog"
import { ErrorState } from "@/shared/ui/empty-state"
import { Input } from "@/shared/ui/input"
import { Skeleton } from "@/shared/ui/skeleton"

/** Configures the two title-pattern rules the task-level report resolves
 * per synced task: display type (falls back to Jira's own issue type) and
 * sub-project label (no fallback -- absent when nothing matches). Lives
 * here, not in Settings, since these rules are read while looking at the
 * report they shape, not while setting up the Jira connection itself. */
export function JiraRulesDialog({ onClose }: { onClose: () => void }) {
  const qc = useQueryClient()
  const settings = useQuery({
    queryKey: ["settings"],
    queryFn: () => api.settings(),
  })
  const [typeRules, setTypeRules] = useState<TaskTypeRule[]>([])
  const [projectRules, setProjectRules] = useState<ProjectPrefixRule[]>([])
  const [message, setMessage] = useState<string | null>(null)

  useEffect(() => {
    if (!settings.data) return
    setTypeRules(settings.data.jira.task_type_rules ?? [])
    setProjectRules(settings.data.jira.project_prefix_rules ?? [])
  }, [settings.data])

  const save = useMutation({
    mutationFn: () => {
      if (!settings.data) throw new Error("Settings not loaded yet")
      return api.updateJira({
        ...settings.data.jira,
        task_type_rules: typeRules.filter(
          (rule) => rule.pattern.trim() && rule.type_label.trim(),
        ),
        project_prefix_rules: projectRules.filter(
          (rule) => rule.pattern.trim() && rule.project_label.trim(),
        ),
      })
    },
    onSuccess: () => {
      setMessage("Rules saved")
      void qc.invalidateQueries({ queryKey: ["settings"] })
      void qc.invalidateQueries({ queryKey: ["task-cost-report"] })
    },
    onError: (e) => setMessage(e instanceof Error ? e.message : "Failed to save"),
  })

  return (
    <Dialog
      open
      title="Configure rules"
      description="Both are checked against a synced task's title, first match wins. Re-run the sync (or wait for the next tick) to see them applied."
      onClose={onClose}
      contentClassName="max-w-xl"
      footer={
        <div className="flex items-center justify-between gap-2">
          <span className="text-xs text-(--color-text-muted)">{message}</span>
          <div className="flex gap-2">
            <Button variant="ghost" onClick={onClose}>
              Close
            </Button>
            <Button
              variant="gradient"
              disabled={save.isPending || settings.isLoading}
              onClick={() => {
                setMessage(null)
                save.mutate()
              }}
            >
              {save.isPending ? "Saving…" : "Save"}
            </Button>
          </div>
        </div>
      }
    >
      {settings.isLoading ? (
        <div className="space-y-2">
          <Skeleton className="h-9 w-full" />
          <Skeleton className="h-9 w-full" />
        </div>
      ) : settings.error ? (
        <ErrorState
          message={settings.error instanceof Error ? settings.error.message : "Failed to load"}
        />
      ) : (
        <div className="space-y-6">
          <RuleList
            title="Task type rules"
            description="A task with no match keeps Jira's own issue type."
            rules={typeRules}
            onChange={setTypeRules}
            valueKey="type_label"
            patternPlaceholder="[Fix]"
            valuePlaceholder="fix"
          />
          <RuleList
            title="Sub-project / module rules"
            description="For a Jira project whose tasks actually span more than one product. A task with no match carries no sub-project label."
            rules={projectRules}
            onChange={setProjectRules}
            valueKey="project_label"
            patternPlaceholder="[PST]"
            valuePlaceholder="Product A"
          />
        </div>
      )}
    </Dialog>
  )
}

function RuleList<K extends string>({
  title,
  description,
  rules,
  onChange,
  valueKey,
  patternPlaceholder,
  valuePlaceholder,
}: {
  title: string
  description: string
  rules: Array<{ pattern: string } & Record<K, string>>
  onChange: (rules: Array<{ pattern: string } & Record<K, string>>) => void
  valueKey: K
  patternPlaceholder: string
  valuePlaceholder: string
}) {
  return (
    <div>
      <h3 className="text-sm font-medium text-(--color-text)">{title}</h3>
      <p className="mt-0.5 mb-2 text-xs text-(--color-text-muted)">{description}</p>
      <div className="space-y-2">
        {rules.map((rule, index) => (
          <div key={index} className="flex items-center gap-2">
            <Input
              className="flex-1"
              value={rule.pattern}
              placeholder={patternPlaceholder}
              onChange={(e) =>
                onChange(
                  rules.map((r, i) => (i === index ? { ...r, pattern: e.target.value } : r)),
                )
              }
            />
            <span className="text-xs text-(--color-text-subtle)">→</span>
            <Input
              className="flex-1"
              value={rule[valueKey]}
              placeholder={valuePlaceholder}
              onChange={(e) =>
                onChange(
                  rules.map((r, i) =>
                    i === index ? { ...r, [valueKey]: e.target.value } : r,
                  ),
                )
              }
            />
            <Button
              variant="ghost"
              size="sm"
              onClick={() => onChange(rules.filter((_, i) => i !== index))}
            >
              Remove
            </Button>
          </div>
        ))}
        <Button
          variant="outline"
          size="sm"
          onClick={() =>
            onChange([...rules, { pattern: "", [valueKey]: "" } as (typeof rules)[number]])
          }
        >
          Add rule
        </Button>
      </div>
    </div>
  )
}
