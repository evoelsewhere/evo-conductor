import assert from "node:assert/strict"
import test from "node:test"

import {
  buildDashboardAttention,
  dashboardAnalyticsHref,
  dashboardResourceTotal,
  dashboardUpdatedAt,
  hasDashboardTelemetry,
} from "./dashboard-model.ts"
import type { ResourceUsageAnalytics } from "@/shared/api/client"

test("dashboardResourceTotal includes every summary resource kind", () => {
  assert.equal(
    dashboardResourceTotal({
      project_name: "Project",
      members_total: 3,
      members_online: 2,
      secrets_active: 1,
      resources: { agents: 1, skills: 2, plugins: 3, workflows: 4 },
      sso_enabled: false,
      presence: {
        clients_seen_recently: 2,
        members_seen_recently: 2,
        threshold_seconds: 180,
        observed_at: "2026-08-18T00:00:00Z",
      },
      realtime: {
        active_owners: 2,
        active_streams: 3,
        scope: "this_node",
      },
      host_metrics: {
        cpu_usage_percent: null,
        memory_used_bytes: null,
        memory_total_bytes: null,
        gpu_usage_percent: null,
        vram_used_bytes: null,
        vram_total_bytes: null,
        sampled_at: "2026-08-18T00:00:00Z",
        scope: "conductor_host",
      },
      feedback: {
        scope: "project",
        count: 0,
        average_rating: null,
        positive_count: 0,
        positive_percent: null,
        distribution: {
          rating_1: 0,
          rating_2: 0,
          rating_3: 0,
          rating_4: 0,
          rating_5: 0,
        },
      },
    }),
    10,
  )
})

test("buildDashboardAttention prioritizes explicit operational conditions", () => {
  const items = buildDashboardAttention(
    analyticsFixture({
      reported_installations: 5,
      installed_installations: 2,
      installed_members: 2,
      pending_installations: 1,
      attention_installations: 2,
      requests: 20,
      resource_uses: 25,
      model_calls: 30,
      tool_calls: 5,
      successes: 16,
      errors: 2,
      blocked: 2,
      tokens_in: 100,
      tokens_out: 50,
      total_tokens: 150,
      estimated_cost_usd_micros: 250_000,
      unpriced_model_calls: 3,
      average_tokens_per_request: 8,
      average_duration_ms: 240,
    }),
    1,
  )

  assert.deepEqual(
    items.map((item) => item.id),
    ["delivery", "errors", "blocked", "unpriced", "members"],
  )
  assert.deepEqual(items[1]?.filter, { status: "error" })
})

test("buildDashboardAttention stays empty without actionable conditions", () => {
  assert.deepEqual(buildDashboardAttention(analyticsFixture(), 0), [])
})

test("hasDashboardTelemetry distinguishes empty and populated telemetry", () => {
  assert.equal(hasDashboardTelemetry(analyticsFixture()), false)
  assert.equal(
    hasDashboardTelemetry(analyticsFixture({ model_calls: 1 })),
    true,
  )
  assert.equal(hasDashboardTelemetry(undefined), false)
})

test("dashboardAnalyticsHref preserves range and filters", () => {
  assert.equal(
    dashboardAnalyticsHref(
      "/app/resources/usage/usage",
      "week",
      "",
      "",
      { model: "gpt-5", provider: "openai" },
    ),
    "/app/resources/usage/usage?range=week&model=gpt-5&provider=openai",
  )
  assert.equal(
    dashboardAnalyticsHref(
      "/app/resources/usage",
      "custom",
      "2026-05-20",
      "2026-08-18",
    ),
    "/app/resources/usage?range=custom&from=2026-05-20&to=2026-08-18",
  )
})

test("dashboardAnalyticsHref maps a one-day dashboard range to the day preset", () => {
  assert.equal(
    dashboardAnalyticsHref(
      "/app/resources/usage",
      "day",
      "2026-08-17",
      "2026-08-18",
    ),
    "/app/resources/usage?range=day",
  )
})

test("dashboardUpdatedAt returns the newest successful query time", () => {
  assert.equal(dashboardUpdatedAt(0, 42, 21), 42)
  assert.equal(dashboardUpdatedAt(0, 0), null)
})

function analyticsFixture(
  totals: Partial<ResourceUsageAnalytics["totals"]> = {},
): ResourceUsageAnalytics {
  return {
    from: "2026-05-20T00:00:00Z",
    to: "2026-08-18T00:00:00Z",
    totals: {
      reported_installations: 0,
      installed_installations: 0,
      installed_members: 0,
      pending_installations: 0,
      attention_installations: 0,
      requests: 0,
      resource_uses: 0,
      model_calls: 0,
      tool_calls: 0,
      successes: 0,
      errors: 0,
      blocked: 0,
      cancelled: 0,
      tokens_in: 0,
      tokens_out: 0,
      cache_read_tokens: 0,
      reasoning_tokens: 0,
      tool_use_tokens: 0,
      total_tokens: 0,
      estimated_cost_usd_micros: 0,
      unpriced_model_calls: 0,
      average_tokens_per_request: 0,
      average_duration_ms: 0,
      ...totals,
    },
    daily: [],
    resources: [],
    members: [],
    models: [],
    roles: [],
    tools: [],
    activity: [],
    activity_total: 0,
    limit: 8,
    offset: 0,
  }
}
