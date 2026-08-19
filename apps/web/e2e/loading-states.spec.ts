import { expect, test, type Page } from "@playwright/test"

import { installDashboardScenario } from "./fixtures/dashboard"

const FIXED_TIME = new Date("2026-08-18T01:30:00.000Z")

test.describe("Loading skeletons", () => {
  test.use({ locale: "en-US", timezoneId: "UTC" })

  test.beforeEach(async ({ page }) => {
    await page.clock.install({ time: FIXED_TIME })
  })

  test("shows a structural shimmer before Dashboard data arrives", async ({
    page,
  }) => {
    await page.emulateMedia({ colorScheme: "light", reducedMotion: "no-preference" })
    const summary = deferred()
    const analytics = deferred()
    await installDashboardScenario(page, "admin", {
      summaryGate: summary.promise,
      analyticsGate: analytics.promise,
    })

    try {
      await page.goto("/app")

      const loading = page
        .locator("[data-slot='loading-state']")
        .filter({ hasText: "Loading dashboard" })
      await expect(loading).toBeVisible()
      await expect
        .poll(() => loading.locator("[data-slot='skeleton']").count())
        .toBeGreaterThanOrEqual(20)
      await expect(
        page.getByRole("heading", { name: "No governed activity in this range" }),
      ).toHaveCount(0)
      await expect(page.getByText("EvoFlux requests", { exact: true })).toHaveCount(0)

      const animationName = await loading
        .locator("[data-slot='skeleton']")
        .first()
        .evaluate((element) => getComputedStyle(element).animationName)
      expect(animationName).toContain("conductor-skeleton-shimmer")

      const signals = page.locator("[data-dashboard-skeleton-panel='signals']")
      const workspace = page.locator("[data-dashboard-skeleton-panel='workspace']")
      await expect
        .poll(async () => {
          const [signalsBox, workspaceBox] = await Promise.all([
            signals.boundingBox(),
            workspace.boundingBox(),
          ])
          return (workspaceBox?.height ?? 0) - (signalsBox?.height ?? 0)
        })
        .toBeGreaterThan(100)

      await expectLoadingScreenshot(page, "dashboard-loading-desktop.png")

      summary.resolve()
      analytics.resolve()
      await expect(loading).toHaveCount(0)
      await expect(page.getByText("EvoFlux requests", { exact: true })).toBeVisible()
      await expect(page.getByText("1,600", { exact: true }).first()).toBeVisible()
    } finally {
      summary.resolve()
      analytics.resolve()
    }
  })

  test("keeps the shimmer static for reduced-motion users", async ({ page }) => {
    await page.emulateMedia({ colorScheme: "dark", reducedMotion: "reduce" })
    const summary = deferred()
    const analytics = deferred()
    await installDashboardScenario(page, "admin", {
      summaryGate: summary.promise,
      analyticsGate: analytics.promise,
    })

    try {
      await page.goto("/app")
      const skeleton = page.locator("[data-slot='skeleton']").first()
      await expect(skeleton).toBeVisible()
      await expect
        .poll(() => skeleton.evaluate((element) => getComputedStyle(element).animationName))
        .toBe("none")
    } finally {
      summary.resolve()
      analytics.resolve()
    }
  })

  test("holds a fast loading frame for one second", async ({ page }) => {
    await page.clock.pauseAt(FIXED_TIME)
    await page.emulateMedia({ colorScheme: "light", reducedMotion: "reduce" })
    const summary = deferred()
    const analytics = deferred()
    await installDashboardScenario(page, "admin", {
      summaryGate: summary.promise,
      analyticsGate: analytics.promise,
    })

    try {
      await page.goto("/app")
      const loading = page
        .locator("[data-slot='loading-state']")
        .filter({ hasText: "Loading dashboard" })
      await expect(loading).toBeVisible()

      const summaryResponse = page.waitForResponse(/\/api\/dashboard$/)
      const analyticsResponse = page.waitForResponse(/\/api\/analytics\/resource-usage/)
      summary.resolve()
      analytics.resolve()
      await Promise.all([summaryResponse, analyticsResponse])
      await page.clock.runFor(0)
      await page.clock.runFor(999)
      await expect(loading).toBeVisible()
      await expect(page.getByText("EvoFlux requests", { exact: true })).toHaveCount(0)

      await page.clock.runFor(1)
      await expect(loading).toHaveCount(0)
      await expect(page.getByText("EvoFlux requests", { exact: true })).toBeVisible()
    } finally {
      summary.resolve()
      analytics.resolve()
    }
  })

  test("does not add another delay after a slow loading cycle", async ({
    page,
  }) => {
    await page.clock.pauseAt(FIXED_TIME)
    await page.emulateMedia({ colorScheme: "light", reducedMotion: "reduce" })
    const summary = deferred()
    const analytics = deferred()
    await installDashboardScenario(page, "admin", {
      summaryGate: summary.promise,
      analyticsGate: analytics.promise,
    })

    try {
      await page.goto("/app")
      const loading = page
        .locator("[data-slot='loading-state']")
        .filter({ hasText: "Loading dashboard" })
      await expect(loading).toBeVisible()

      await page.clock.runFor(1_250)
      await expect(loading).toBeVisible()

      const summaryResponse = page.waitForResponse(/\/api\/dashboard$/)
      const analyticsResponse = page.waitForResponse(/\/api\/analytics\/resource-usage/)
      summary.resolve()
      analytics.resolve()
      await Promise.all([summaryResponse, analyticsResponse])
      await page.clock.runFor(0)

      await expect(loading).toHaveCount(0, { timeout: 1_000 })
      await expect(page.getByText("EvoFlux requests", { exact: true })).toBeVisible()
    } finally {
      summary.resolve()
      analytics.resolve()
    }
  })

  test("holds a fast error without rendering skeleton and error together", async ({
    page,
  }) => {
    await page.clock.pauseAt(FIXED_TIME)
    await page.emulateMedia({ colorScheme: "light", reducedMotion: "reduce" })
    const summary = deferred()
    const analytics = deferred()
    await installDashboardScenario(page, "admin", {
      summaryGate: summary.promise,
      analyticsGate: analytics.promise,
      summaryFailure: { status: 404, message: "Dashboard snapshot unavailable" },
      analyticsFailure: { status: 404, message: "Dashboard analytics unavailable" },
    })

    try {
      await page.goto("/app")
      const loading = page
        .locator("[data-slot='loading-state']")
        .filter({ hasText: "Loading dashboard" })
      await expect(loading).toBeVisible()

      const summaryResponse = page.waitForResponse(/\/api\/dashboard$/)
      const analyticsResponse = page.waitForResponse(/\/api\/analytics\/resource-usage/)
      summary.resolve()
      analytics.resolve()
      await Promise.all([summaryResponse, analyticsResponse])
      await page.clock.runFor(999)

      await expect(loading).toBeVisible()
      await expect(page.getByRole("alert")).toHaveCount(0)

      await page.clock.runFor(1)
      await expect(loading).toHaveCount(0)
      await expect(
        page.getByRole("alert").filter({ hasText: "Dashboard snapshot unavailable" }),
      ).toBeVisible()
      await expect(
        page.getByRole("alert").filter({ hasText: "Dashboard analytics unavailable" }),
      ).toBeVisible()
    } finally {
      summary.resolve()
      analytics.resolve()
    }
  })

  test("keeps populated data visible during a background refresh", async ({
    page,
  }) => {
    await page.clock.pauseAt(FIXED_TIME)
    await page.emulateMedia({ colorScheme: "light", reducedMotion: "reduce" })
    await installDashboardScenario(page, "admin")

    const initialSummary = page.waitForResponse(/\/api\/dashboard$/)
    const initialAnalytics = page.waitForResponse(/\/api\/analytics\/resource-usage/)
    await page.goto("/app")
    await Promise.all([initialSummary, initialAnalytics])
    await page.clock.runFor(1_000)
    await expect(page.getByText("EvoFlux requests", { exact: true })).toBeVisible()
    await expect(page.getByText("1,600", { exact: true }).first()).toBeVisible()

    const summary = deferred()
    const analytics = deferred()
    await page.route(/^https?:\/\/[^/]+\/api\/dashboard$/, async (route) => {
      await summary.promise
      await route.fallback()
    })
    await page.route(
      /^https?:\/\/[^/]+\/api\/analytics\/resource-usage(?:\?.*)?$/,
      async (route) => {
        await analytics.promise
        await route.fallback()
      },
    )

    try {
      const summaryRequest = page.waitForRequest(/\/api\/dashboard$/)
      const analyticsRequest = page.waitForRequest(
        /\/api\/analytics\/resource-usage/,
      )
      const pendingResponse = page.waitForResponse(
        /\/api\/members\/pending\/count$/,
      )
      await page.getByRole("button", { name: "Refresh" }).click()
      await Promise.all([summaryRequest, analyticsRequest])

      await expect(page.getByText("Updating…", { exact: true })).toBeVisible()
      await expect(page.getByText("1,600", { exact: true }).first()).toBeVisible()
      await expect(page.locator("[data-slot='loading-state']")).toHaveCount(0)

      await page.clock.runFor(1_250)
      await expect(page.getByText("1,600", { exact: true }).first()).toBeVisible()
      await expect(page.locator("[data-slot='loading-state']")).toHaveCount(0)

      const summaryResponse = page.waitForResponse(/\/api\/dashboard$/)
      const analyticsResponse = page.waitForResponse(/\/api\/analytics\/resource-usage/)
      summary.resolve()
      analytics.resolve()
      await Promise.all([summaryResponse, analyticsResponse, pendingResponse])
      await page.clock.runFor(0)
      await expect(page.getByText("Updating…", { exact: true })).toHaveCount(0)
    } finally {
      summary.resolve()
      analytics.resolve()
    }
  })

  test("keeps the loading frame stable on mobile", async ({ page }) => {
    await page.setViewportSize({ width: 390, height: 844 })
    await page.emulateMedia({ colorScheme: "light", reducedMotion: "reduce" })
    const summary = deferred()
    const analytics = deferred()
    await installDashboardScenario(page, "admin", {
      summaryGate: summary.promise,
      analyticsGate: analytics.promise,
    })

    try {
      await page.goto("/app")
      await expect(
        page
          .locator("[data-slot='loading-state']")
          .filter({ hasText: "Loading dashboard" }),
      ).toBeVisible()
      const frameBefore = await page.locator("#main-content").boundingBox()
      await expect
        .poll(() =>
          page.evaluate(() =>
            Math.max(
              0,
              document.documentElement.scrollWidth -
                document.documentElement.clientWidth,
            ),
          ),
        )
        .toBeLessThanOrEqual(1)
      await expectLoadingScreenshot(page, "dashboard-loading-mobile.png")
      summary.resolve()
      analytics.resolve()
      await expect(page.getByText("EvoFlux requests", { exact: true })).toBeVisible()
      const frameAfter = await page.locator("#main-content").boundingBox()
      expect(frameAfter?.x).toBe(frameBefore?.x)
      expect(frameAfter?.width).toBe(frameBefore?.width)
      await expect
        .poll(() =>
          page.evaluate(() =>
            Math.max(
              0,
              document.documentElement.scrollWidth -
                document.documentElement.clientWidth,
            ),
          ),
        )
        .toBeLessThanOrEqual(1)
    } finally {
      summary.resolve()
      analytics.resolve()
    }
  })

  test("uses structural loaders on Members and the resource catalog", async ({
    page,
  }) => {
    await page.emulateMedia({ colorScheme: "light", reducedMotion: "reduce" })
    await installDashboardScenario(page, "admin")
    const members = deferred()
    const resources = deferred()

    await page.route(
      /^https?:\/\/[^/]+\/api\/members(?:\?.*)?$/,
      async (route) => {
        await members.promise
        await route.fulfill({
          contentType: "application/json",
          body: JSON.stringify({ items: [], total: 0, page: 1, limit: 25 }),
        })
      },
    )
    await page.route(/^https?:\/\/[^/]+\/api\/(?:tags|sub-roles)$/, (route) =>
      route.fulfill({ contentType: "application/json", body: "[]" }),
    )
    await page.route(
      /^https?:\/\/[^/]+\/api\/resources(?:\?.*)?$/,
      async (route) => {
        await resources.promise
        await route.fulfill({ contentType: "application/json", body: "[]" })
      },
    )

    try {
      await page.goto("/app/members")
      const membersLoading = page
        .locator("[data-slot='loading-state']")
        .filter({ hasText: "Loading members" })
      await expect(membersLoading).toBeVisible()
      await expect(page.getByText("No members match", { exact: true })).toHaveCount(0)
      members.resolve()
      await expect(membersLoading).toHaveCount(0)
      await expect(page.getByText("No members match", { exact: true })).toBeVisible()

      await page.goto("/app/resources")
      const catalogLoading = page
        .locator("[data-slot='loading-state']")
        .filter({ hasText: "Loading resource catalog" })
      await expect(catalogLoading).toBeVisible()
      await expect(
        page.getByText("Build your governed catalog", { exact: true }),
      ).toHaveCount(0)
      resources.resolve()
      await expect(catalogLoading).toHaveCount(0)
      await expect(
        page.getByText("Build your governed catalog", { exact: true }),
      ).toBeVisible()
    } finally {
      members.resolve()
      resources.resolve()
    }
  })
})

function deferred() {
  let resolve!: () => void
  const promise = new Promise<void>((done) => {
    resolve = done
  })
  return { promise, resolve }
}

async function expectLoadingScreenshot(page: Page, name: string) {
  await page.evaluate(async () => {
    await document.fonts.ready
  })
  await expect(page).toHaveScreenshot(name, {
    animations: "disabled",
    caret: "hide",
    fullPage: true,
    maxDiffPixels: 100,
    scale: "css",
  })
}
