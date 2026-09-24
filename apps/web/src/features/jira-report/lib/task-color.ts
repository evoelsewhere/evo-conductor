type BadgeTone = "success" | "info" | "warning" | "danger" | "neutral" | "accent"

// Two vocabularies share this one badge: a project's own `task_type_rules`
// labels (create/review/fix/...) when a rule matches the title, and Jira's
// raw issue type (Epic/Story/Task/Subtask/Bug) when none does -- a task
// synced before its rules were configured, or one no rule was ever written
// for, falls back to the raw type rather than going unstyled.
const TYPE_TONE: Record<string, BadgeTone> = {
  create: "success",
  add: "success",
  feature: "success",
  story: "success",
  review: "info",
  fix: "danger",
  bug: "danger",
  update: "warning",
  improve: "warning",
  chore: "neutral",
  task: "neutral",
  subtask: "neutral",
  epic: "accent",
}

export function typeTone(resolvedType: string | null): BadgeTone {
  if (!resolvedType) return "neutral"
  return TYPE_TONE[resolvedType.trim().toLowerCase()] ?? "neutral"
}
