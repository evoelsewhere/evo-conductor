import assert from "node:assert/strict"
import test from "node:test"

import type { ResourceUsageAnalytics, ResourceUsageBreakdown } from "@/shared/api/client"

import { buildCsv } from "./resource-analytics-export.ts"

/** One request that used two resources: each row carries its whole cost. */
function resourceRow(name: string): ResourceUsageBreakdown {
  return {
    resource_id: `resource-${name}`,
    version_id: `version-${name}`,
    kind: "agent",
    name,
    version: "1.0.0",
    relation: "executing_agent",
    uses: 1,
    members: 1,
    requests: 1,
    successes: 1,
    errors: 0,
    model_calls: 1,
    tool_calls: 0,
    total_tokens: 150,
    estimated_cost_usd_micros: 1250,
    last_used_at: "2026-09-09T00:00:00Z",
  }
}

const analytics = {
  from: "2026-09-01T00:00:00Z",
  to: "2026-09-09T00:00:00Z",
  scope: "governed",
  totals: {},
  daily: [],
  resources: [resourceRow("reviewer"), resourceRow("formatter")],
  members: [],
  models: [],
  roles: [],
  tools: [],
  activity: [],
  activity_total: 0,
  limit: 50,
  offset: 0,
} as unknown as ResourceUsageAnalytics

function resourceBlock(csv: string) {
  return csv.split("\n").filter((line) => line.startsWith("resource,"))
}

// A spreadsheet will sum any column put in front of it, so the file has to
// say that these rows overlap.
test("the resource block warns that its rows do not sum to the totals", () => {
  const csv = buildCsv(analytics, "All resources", "2026-09-09T12:00:00Z")
  const note = csv.split("\n").find((line) => line.startsWith("note,"))
  assert.ok(note, "the resource block must carry a note")
  assert.match(note, /counted in full against each/)
  assert.match(note, /do not sum/)
})

test("the resource cost column is named for what it measures", () => {
  const csv = buildCsv(analytics, "All resources", "2026-09-09T12:00:00Z")
  const header = csv.split("\n").find((line) => line.startsWith("resources,"))
  assert.ok(header, "the resource block must have a header")
  assert.ok(
    header.includes("attributed_request_cost_usd_micros"),
    "a bare estimated_cost_usd_micros reads as this resource's own cost",
  )
})

// The overlap itself: two rows, each holding the same request's full cost.
test("a request used by two resources is exported in full against each", () => {
  const csv = buildCsv(analytics, "All resources", "2026-09-09T12:00:00Z")
  const rows = resourceBlock(csv)
  assert.equal(rows.length, 2)
  for (const row of rows) {
    assert.ok(row.includes(",1250,"), `expected the whole request cost in ${row}`)
  }
})
