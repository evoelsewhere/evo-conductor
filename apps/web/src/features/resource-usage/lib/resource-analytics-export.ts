import type { ResourceUsageAnalytics } from "@/shared/api/client"

type ReportFormat = "csv" | "json"

export function exportResourceAnalytics(
  data: ResourceUsageAnalytics,
  format: ReportFormat,
  scopeLabel: string,
) {
  const generatedAt = new Date().toISOString()
  const filename = `conductor-${slugify(scopeLabel)}-${data.from}-${data.to}.${format}`
  const content = format === "json"
    ? JSON.stringify(
        {
          report: {
            name: `${scopeLabel} analytics`,
            generated_at: generatedAt,
            from: data.from,
            to: data.to,
            privacy: "Operational metadata only; prompts, responses and tool arguments are excluded.",
          },
          data,
        },
        null,
        2,
      )
    : buildCsv(data, scopeLabel, generatedAt)

  const blob = new Blob([content], {
    type: format === "json" ? "application/json;charset=utf-8" : "text/csv;charset=utf-8",
  })
  const href = URL.createObjectURL(blob)
  const anchor = document.createElement("a")
  anchor.href = href
  anchor.download = filename
  anchor.click()
  window.setTimeout(() => URL.revokeObjectURL(href), 0)
}

function buildCsv(
  data: ResourceUsageAnalytics,
  scopeLabel: string,
  generatedAt: string,
) {
  const rows: Array<Array<string | number | null>> = [
    ["report", "scope", "from", "to", "generated_at"],
    ["metadata", scopeLabel, data.from, data.to, generatedAt],
    [],
    [
      "daily",
      "date",
      "requests",
      "successes",
      "errors",
      "blocked",
      "cancelled",
      "tokens_in",
      "tokens_out",
      "cache_read_tokens",
      "reasoning_tokens",
      "tool_use_tokens",
      "estimated_cost_usd_micros",
      "unpriced_model_calls",
    ],
    ...data.daily.map((item) => [
      "daily",
      item.date,
      item.requests,
      item.successes,
      item.errors,
      item.blocked,
      item.cancelled,
      item.tokens_in,
      item.tokens_out,
      item.cache_read_tokens,
      item.reasoning_tokens,
      item.tool_use_tokens,
      item.estimated_cost_usd_micros,
      item.unpriced_model_calls,
    ]),
    [],
    [
      "resources",
      "name",
      "kind",
      "version",
      "relation",
      "uses",
      "members",
      "requests",
      "successes",
      "errors",
      "model_calls",
      "tool_calls",
      "total_tokens",
      "estimated_cost_usd_micros",
      "last_used_at",
    ],
    ...data.resources.map((item) => [
      "resource",
      item.name,
      item.kind,
      item.version,
      item.relation,
      item.uses,
      item.members,
      item.requests,
      item.successes,
      item.errors,
      item.model_calls,
      item.tool_calls,
      item.total_tokens,
      item.estimated_cost_usd_micros,
      item.last_used_at,
    ]),
    [],
    ["members", "display_name", "email", "role", "requests", "resource_uses", "total_tokens", "estimated_cost_usd_micros"],
    ...data.members.map((item) => [
      "member",
      item.display_name,
      item.email,
      item.primary_role,
      item.requests,
      item.resource_uses,
      item.total_tokens,
      item.estimated_cost_usd_micros,
    ]),
    [],
    ["models", "provider", "model", "calls", "total_tokens", "estimated_cost_usd_micros", "unpriced_calls"],
    ...data.models.map((item) => [
      "model",
      item.provider,
      item.model,
      item.calls,
      item.total_tokens,
      item.estimated_cost_usd_micros,
      item.unpriced_calls,
    ]),
    [],
    ["tools", "tool_name", "category", "calls", "successes", "errors", "blocked", "cancelled", "average_duration_ms", "last_used_at"],
    ...data.tools.map((item) => [
      "tool",
      item.tool_name,
      item.category,
      item.calls,
      item.successes,
      item.errors,
      item.blocked,
      item.cancelled,
      item.average_duration_ms,
      item.last_used_at,
    ]),
  ]

  return rows.map((row) => row.map(csvCell).join(",")).join("\n")
}

function csvCell(value: string | number | null) {
  if (value == null) return ""
  const text = String(value)
  return /[",\n]/.test(text) ? `"${text.replaceAll('"', '""')}"` : text
}

function slugify(value: string) {
  return value
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-|-$/g, "") || "resources"
}
