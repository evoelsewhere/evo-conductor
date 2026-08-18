import { expect, test, type Page } from "@playwright/test"

import {
  DASHBOARD_MEMBER_EMAIL,
  DASHBOARD_MEMBER_ID,
  DASHBOARD_MEMBER_NAME,
  installDashboardScenario,
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

    await expect(metric(page, "SSE streams · this node")).toContainText("11")
    await expect(metric(page, "Governed requests")).toContainText("1,200")
    await expect(metric(page, "Success rate")).toContainText("80%")
    await expect(metric(page, "Average request duration")).toContainText("1.4 s")
    await expect(metric(page, "Estimated cost")).toContainText("$12.5")
    await expect(metric(page, "Delivery attention")).toContainText("1")

    const liveOperations = cardForHeading(page, "Live operations")
    await expect(
      liveOperations.getByText("Members seen recently", { exact: true }),
    ).toBeVisible()
    await expect(
      liveOperations.getByText("Clients seen recently", { exact: true }),
    ).toBeVisible()
    await expect(liveOperations.getByText("CPU", { exact: true })).toBeVisible()
    await expect(liveOperations.getByText("Memory", { exact: true })).toBeVisible()
    await expect(liveOperations.getByText("GPU", { exact: true })).toBeVisible()
    await expect(liveOperations.getByText("VRAM", { exact: true })).toBeVisible()
    await expect(
      liveOperations.getByRole("progressbar", { name: "CPU usage" }),
    ).toHaveAttribute("aria-valuenow", "37.4")
    await expect(
      liveOperations.getByRole("progressbar", { name: "Memory usage" }),
    ).toHaveAttribute("aria-valuenow", "50")
    await expect(
      liveOperations.getByRole("progressbar", { name: "GPU usage" }),
    ).toHaveCount(0)
    await expect(
      liveOperations.getByRole("progressbar", { name: "VRAM usage" }),
    ).toHaveCount(0)
    await expect(
      liveOperations.getByText("Not reported", { exact: true }),
    ).toHaveCount(2)

    await expect(
      page.getByRole("heading", { name: "Top signals", level: 2 }),
    ).toBeVisible()
    for (const heading of ["Top members", "Resources", "Models", "Tools"]) {
      await expect(
        page.getByRole("heading", { name: heading, level: 3 }),
      ).toBeVisible()
    }
    await expect(page.getByText(DASHBOARD_MEMBER_NAME, { exact: true })).toBeVisible()
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
      page.getByRole("heading", { name: "Top members", level: 3 }),
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

    const liveOperations = cardForHeading(page, "Live operations")
    await expect(liveOperations).toBeVisible()
    for (const label of ["CPU", "Memory", "GPU", "VRAM"]) {
      await expect(liveOperations.getByText(label, { exact: true })).toBeVisible()
    }
    await expect(
      liveOperations.getByText("Not reported", { exact: true }),
    ).toHaveCount(4)
    await expect(liveOperations.getByRole("progressbar")).toHaveCount(0)
    await expect(liveOperations.getByText("0%", { exact: true })).toHaveCount(0)
    await expect(liveOperations.getByText(/0\s*(?:B|KB|MB|GB)/, { exact: true })).toHaveCount(0)
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
      page.getByRole("heading", { name: "Top signals", level: 2 }),
    ).toBeVisible()
    await expect(
      page.getByRole("heading", { name: "Role & workspace", level: 2 }),
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
  return page.getByText(label, { exact: true }).first().locator("../..")
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
