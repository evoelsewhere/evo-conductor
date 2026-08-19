import { expect, test, type Locator, type Page } from "@playwright/test"

import {
  DASHBOARD_MEMBER_EMAIL,
  DASHBOARD_MEMBER_ID,
  DASHBOARD_MEMBER_NAME,
  installDashboardScenario,
  POPULATED_DASHBOARD_ANALYTICS,
  UNSUPPORTED_RUNTIME_DASHBOARD_SUMMARY,
} from "./fixtures/dashboard"

test.describe("Dashboard mission control", () => {
  test.use({ locale: "en-US", timezoneId: "UTC" })

  test.beforeEach(async ({ page }) => {
    await page.clock.setFixedTime(new Date("2026-08-18T01:30:00.000Z"))
    await page.emulateMedia({ colorScheme: "light", reducedMotion: "reduce" })
  })

  test("renders populated Admin monitoring and navigation", async ({ page }) => {
    const requests = await installDashboardScenario(page, "admin")

    await page.goto("/app")

    await expect(
      page.getByRole("heading", { name: "Dashboard", level: 1 }),
    ).toBeVisible()
    await expect(
      page.getByRole("group", { name: "Dashboard time range" }),
    ).toBeVisible()
    await expect(
      page.getByRole("button", { name: "30 days", exact: true }),
    ).toHaveAttribute("aria-pressed", "true")

    const range = page.getByRole("group", { name: "Dashboard time range" })
    await range.getByRole("button", { name: "24 hours", exact: true }).click()
    await expect(
      range.getByRole("button", { name: "24 hours", exact: true }),
    ).toHaveAttribute("aria-pressed", "true")
    await expect.poll(() => requests.analytics.at(-1) ?? "").toContain(
      "from=2026-08-17T01%3A30%3A00.000Z",
    )
    await expect.poll(() => requests.analytics.at(-1) ?? "").toContain(
      "to=2026-08-18T01%3A30%3A00.000Z",
    )
    await range.getByRole("button", { name: "30 days", exact: true }).click()

    await expect(metric(page, "SSE streams · this node")).toContainText("11")
    await expect(metric(page, "EvoFlux requests")).toContainText("1,600")
    await expect(metric(page, "EvoFlux requests")).toContainText(
      "1,200 governed · 75% attributed",
    )
    await expect(metric(page, "Success rate")).toContainText("80%")
    await expect(metric(page, "Average duration")).toContainText("1.4 s")
    await expect(metric(page, "Estimated cost")).toContainText("$12.5")
    await expect(metric(page, "Delivery attention")).toContainText("1")

    const liveOperations = cardForHeading(page, "Live operations")
    await expect(
      liveOperations.getByText("Members seen recently", { exact: true }),
    ).toBeVisible()
    await expect(
      liveOperations.getByText("Clients seen recently", { exact: true }),
    ).toBeVisible()
    const operationalSummary = page.getByRole("region", {
      name: "Operational summary",
    })
    for (const label of ["CPU", "Memory", "GPU", "VRAM"]) {
      await expect(hostMetric(operationalSummary, label)).toBeVisible()
    }
    await expect(operationalSummary).toContainText("37.4%")
    await expect(operationalSummary).toContainText("8 GB / 16 GB")
    await expect(
      operationalSummary.getByText("Not reported", { exact: true }),
    ).toHaveCount(2)

    await expect(
      page.getByRole("heading", { name: "Member activity", level: 2 }),
    ).toBeVisible()
    await expect(
      page.getByRole("heading", { name: "Usage breakdown", level: 2 }),
    ).toBeVisible()
    for (const heading of ["Resources", "Models", "Tools"]) {
      await expect(
        page.getByRole("heading", { name: heading, level: 3 }),
      ).toBeVisible()
    }
    await expect(
      page.getByRole("link", { name: DASHBOARD_MEMBER_NAME, exact: true }).first(),
    ).toBeVisible()
    const memberActivity = page.getByRole("link", {
      name: `Inspect governed activity for ${DASHBOARD_MEMBER_NAME}`,
    })
    await expect(memberActivity).toHaveAttribute(
      "href",
      new RegExp(`member_id=${DASHBOARD_MEMBER_ID}`),
    )
    const memberRow = page.getByRole("row").filter({ hasText: DASHBOARD_MEMBER_NAME })
    await expect(memberRow).toContainText("2")
    await expect(memberRow).toContainText("420")
    await expect(memberRow).toContainText("560 / 315")
    await expect(memberRow).toContainText("2.5M")
    await expect(memberRow).toContainText("$4.70")
    await expect(memberRow).toContainText("Aug 18, 01:26 AM")
    await expect(page.getByText("Research copilot", { exact: true })).toBeVisible()
    await expect(page.getByText("gpt-5", { exact: true })).toBeVisible()
    await expect(page.getByText("web.search", { exact: true })).toBeVisible()

    await expect(
      page.getByRole("heading", { name: "Role usage", level: 3 }),
    ).toBeVisible()
    await expect(page.getByText("Recorded at ingestion", { exact: true })).toBeVisible()
    await expect(
      page.getByRole("heading", { name: "Feedback", level: 3 }),
    ).toBeVisible()
    await expect(
      page.getByRole("heading", { name: "Navigate", level: 3 }),
    ).toBeVisible()

    await expect.poll(() => requests.dashboard.length).toBeGreaterThan(0)
    await expect.poll(() => requests.analytics.length).toBeGreaterThan(0)
    expect(requests.unexpected).toEqual([])

    await expectDashboardScreenshot(page, "dashboard-admin-populated-desktop.png")
  })

  test("keeps member identity out of the Contributor dashboard", async ({ page }) => {
    const requests = await installDashboardScenario(page, "contribute")

    await page.goto("/app")

    await expect(
      page.getByRole("heading", { name: "Dashboard", level: 1 }),
    ).toBeVisible()
    await expect(
      page.getByRole("heading", { name: "Member activity", level: 2 }),
    ).toHaveCount(0)
    await expect(
      page.getByText(DASHBOARD_MEMBER_NAME, { exact: true }),
    ).toHaveCount(0)
    await expect(
      page.getByText(DASHBOARD_MEMBER_EMAIL, { exact: true }),
    ).toHaveCount(0)
    await expect(
      page.locator(`a[href="/app/members/${DASHBOARD_MEMBER_ID}"]`),
    ).toHaveCount(0)

    for (const heading of ["Resources", "Models", "Tools", "Role usage"]) {
      await expect(page.getByRole("heading", { name: heading })).toBeVisible()
    }
    await expect(page.getByText("Research copilot", { exact: true })).toBeVisible()
    await expect(page.getByText("Recorded at ingestion", { exact: true })).toBeVisible()
    await expect(page.getByText("Owned resources", { exact: true })).toBeVisible()
    await expect(page.getByText("Project scope", { exact: true })).toHaveCount(0)
    expect(requests.pendingCount).toBe(0)
    expect(requests.unexpected).toEqual([])
  })

  test("shows ordinary EvoFlux requests when none used governed resources", async ({
    page,
  }) => {
    const analytics = {
      ...POPULATED_DASHBOARD_ANALYTICS,
      totals: {
        ...POPULATED_DASHBOARD_ANALYTICS.totals,
        all_requests: 12,
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
        total_tokens: 0,
        estimated_cost_usd_micros: 0,
        unpriced_model_calls: 0,
        average_tokens_per_request: 0,
        average_duration_ms: 0,
      },
      daily: [],
      resources: [],
      members: [],
      models: [],
      roles: [],
      tools: [],
      activity: [],
      activity_total: 0,
    }
    await installDashboardScenario(page, "admin", { analytics })

    await page.goto("/app")

    await expect(metric(page, "EvoFlux requests")).toContainText("12")
    await expect(metric(page, "EvoFlux requests")).toContainText(
      "0 governed · 0% attributed",
    )
    await expect(
      page.getByText("No governed activity in this range", { exact: true }),
    ).toBeVisible()
    await expect(
      page.getByText(
        "12 EvoFlux requests were received, but none used an Agent, Skill or Plugin governed by Conductor.",
      ),
    ).toBeVisible()
  })

  test("redirects User away before Dashboard queries run", async ({ page }) => {
    const requests = await installDashboardScenario(page, "user")

    await page.goto("/app")

    await expect(page).toHaveURL(/\/app\/resources$/)
    await expect(
      page.getByRole("heading", { name: "Resource catalog", level: 1 }),
    ).toBeVisible()
    await expect(
      page.getByRole("heading", { name: "Dashboard", level: 1 }),
    ).toHaveCount(0)
    await expect(
      page.locator("aside").getByRole("link", { name: "Dashboard", exact: true }),
    ).toHaveCount(0)
    expect(requests.dashboard).toEqual([])
    expect(requests.analytics).toEqual([])
    expect(requests.unexpected).toEqual([])
  })

  test("labels unsupported host metrics without inventing zero utilization", async ({
    page,
  }) => {
    await installDashboardScenario(page, "admin", {
      summary: UNSUPPORTED_RUNTIME_DASHBOARD_SUMMARY,
    })

    await page.goto("/app")

    const operationalSummary = page.getByRole("region", {
      name: "Operational summary",
    })
    for (const label of ["CPU", "Memory", "GPU", "VRAM"]) {
      await expect(hostMetric(operationalSummary, label)).toBeVisible()
    }
    await expect(
      operationalSummary.getByText("Not reported", { exact: true }),
    ).toHaveCount(4)
    await expect(operationalSummary.getByText("0%", { exact: true })).toHaveCount(0)
    await expect(operationalSummary.getByText(/0\s*(?:B|KB|MB|GB)/, { exact: true })).toHaveCount(0)
  })

  test("keeps the populated Dashboard usable at mobile width", async ({ page }) => {
    await page.setViewportSize({ width: 390, height: 844 })
    const requests = await installDashboardScenario(page, "admin")

    await page.goto("/app")

    await expect(
      page.getByRole("heading", { name: "Dashboard", level: 1 }),
    ).toBeVisible()
    const range = page.getByRole("group", { name: "Dashboard time range" })
    await expect(range).toBeVisible()
    await range.getByRole("button", { name: "7 days", exact: true }).click()
    await expect(
      range.getByRole("button", { name: "7 days", exact: true }),
    ).toHaveAttribute("aria-pressed", "true")
    await expect.poll(() => requests.analytics.length).toBeGreaterThanOrEqual(2)

    await expect(
      page.getByRole("heading", { name: "Live operations", level: 2 }),
    ).toBeVisible()
    await expect(
      page.getByRole("heading", { name: "Usage breakdown", level: 2 }),
    ).toBeVisible()
    await expect(
      page.getByRole("heading", { name: "Project context", level: 2 }),
    ).toBeVisible()
    await expect
      .poll(() =>
        page.evaluate(() =>
          Math.max(
            0,
            document.documentElement.scrollWidth - document.documentElement.clientWidth,
          ),
        ),
      )
      .toBeLessThanOrEqual(1)

    const openMenu = page.getByRole("button", { name: "Open menu" })
    await expect(openMenu).toBeVisible()
    await openMenu.click()
    const navigation = page.getByRole("dialog", { name: "Project navigation" })
    await expect(
      navigation.getByRole("link", { name: "Dashboard", exact: true }),
    ).toBeVisible()
    await navigation.getByRole("button", { name: "Close menu" }).click()

    expect(requests.unexpected).toEqual([])
    await expectDashboardScreenshot(page, "dashboard-admin-populated-mobile.png")
  })
})

function metric(page: Page, label: string) {
  return page.locator(`[data-dashboard-metric="${label}"]`)
}

function hostMetric(scope: Locator, label: string) {
  return scope.locator(`[data-dashboard-host-metric="${label}"]`)
}

function cardForHeading(page: Page, heading: string) {
  return page
    .getByRole("heading", { name: heading })
    .locator("xpath=ancestor::*[@data-slot='card'][1]")
}

async function expectDashboardScreenshot(page: Page, name: string) {
  await page.evaluate(async () => {
    await document.fonts.ready
  })
  await expect(page).toHaveScreenshot(name, {
    animations: "disabled",
    caret: "hide",
    fullPage: true,
    // Keep a tiny absolute budget for SVG chart-label rasterization. A layout
    // shift across either full-page image still exceeds this by a wide margin.
    maxDiffPixels: 100,
    scale: "css",
  })
}
