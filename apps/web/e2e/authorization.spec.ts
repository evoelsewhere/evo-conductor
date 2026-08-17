import {
  expect,
  test,
  type APIRequestContext,
  type APIResponse,
  type Page,
} from "@playwright/test"

const ADMIN = {
  email: "playwright.admin@example.test",
  displayName: "Playwright Admin",
  password: "AdminPass!2026",
}

const TARGET = {
  email: "playwright.member@example.test",
  displayName: "Playwright Member",
}

type PrimaryRole = "admin" | "contribute" | "user"

type Member = {
  id: string
  primary_role: PrimaryRole
  status: "pending" | "invited" | "active" | "disabled"
  must_change_password: boolean
}

type CreatedMember = {
  user: Member
  temporary_password: string
}

function appBaseUrl(testBaseUrl: string | undefined): string {
  if (!testBaseUrl) {
    throw new Error("Playwright use.baseURL must be configured for the E2E suite")
  }
  return testBaseUrl
}

function apiUrl(baseUrl: string, path: string): string {
  return new URL(`/api${path}`, baseUrl).toString()
}

function bearer(token: string) {
  return { Authorization: `Bearer ${token}` }
}

async function jsonFromOk<T>(
  response: APIResponse,
  operation: string,
): Promise<T> {
  const body = await response.text()
  if (!response.ok()) {
    throw new Error(`${operation} failed with ${response.status()}: ${body}`)
  }
  return JSON.parse(body) as T
}

async function setupProject(page: Page, baseUrl: string) {
  await page.goto("/")
  await expect(
    page.getByRole("heading", { name: "Set up your Conductor" }),
  ).toBeVisible()

  await page.getByLabel("Project name").fill("playwright-authorization")
  await page.getByLabel("Display name").fill("Playwright Authorization")
  await page.getByRole("button", { name: "Continue" }).click()

  await expect(page.getByLabel("Bind host")).toBeVisible()
  await page.getByLabel("Public URL").fill(baseUrl.replace(/\/$/, ""))
  await page.getByRole("button", { name: "Continue" }).click()

  await page.getByLabel("Admin display name").fill(ADMIN.displayName)
  await page.getByLabel("Admin email").fill(ADMIN.email)
  await page.getByLabel("Admin password").fill(ADMIN.password)
  await page.getByRole("button", { name: "Continue" }).click()

  await expect(
    page.getByRole("switch", { name: "Enable SSO" }),
  ).not.toBeChecked()
  await page.getByRole("button", { name: "Finish & publish" }).click()
  await expect(page).toHaveURL(/\/login$/)
}

async function login(page: Page, email: string, password: string) {
  await page.goto("/login")
  await page.getByLabel("Email").fill(email)
  await page.getByLabel("Password").fill(password)
  await page.getByRole("button", { name: "Continue" }).click()
  await expect(page).toHaveURL(/\/app(?:\/.*)?$/)
  await expect(page.locator("[data-shell]")).toBeVisible()
}

async function sessionToken(page: Page): Promise<string> {
  const token = await page.evaluate(() =>
    window.sessionStorage.getItem("conductor.token"),
  )
  if (!token) throw new Error("Expected an authenticated browser session")
  return token
}

async function updateRole(
  request: APIRequestContext,
  baseUrl: string,
  adminToken: string,
  memberId: string,
  role: PrimaryRole,
) {
  const response = await request.patch(apiUrl(baseUrl, `/members/${memberId}`), {
    headers: bearer(adminToken),
    data: { primary_role: role },
  })
  const updated = await jsonFromOk<Member>(
    response,
    `update member role to ${role}`,
  )
  expect(updated.primary_role).toBe(role)
}

test("REQ-004 projects role changes into the console without stale authority", async ({
  browser,
  page: adminPage,
  request,
}, testInfo) => {
  const baseUrl = appBaseUrl(testInfo.project.use.baseURL)

  await test.step("complete first-run setup and authenticate the administrator", async () => {
    await setupProject(adminPage, baseUrl)
    await login(adminPage, ADMIN.email, ADMIN.password)

    await adminPage.keyboard.press("Tab")
    await expect(
      adminPage.getByRole("link", { name: "Skip to content" }),
    ).toBeFocused()
    await adminPage.locator("#main-content").focus()
    await expect(adminPage.locator("#main-content")).toBeFocused()
    await adminPage.evaluate(() => {
      if (document.activeElement instanceof HTMLElement) {
        document.activeElement.blur()
      }
    })

    const screenshotPath = testInfo.outputPath("authenticated-shell-desktop.png")
    await adminPage.screenshot({ path: screenshotPath, fullPage: true })
    await testInfo.attach("authenticated-shell-desktop", {
      path: screenshotPath,
      contentType: "image/png",
    })
  })

  const adminToken = await sessionToken(adminPage)
  const created = await test.step("provision an active User", async () => {
    const createResponse = await request.post(apiUrl(baseUrl, "/members"), {
      headers: bearer(adminToken),
      data: {
        email: TARGET.email,
        display_name: TARGET.displayName,
        primary_role: "user",
        sub_role_ids: [],
        tag_ids: [],
      },
    })
    const member = await jsonFromOk<CreatedMember>(
      createResponse,
      "create target member",
    )
    expect(member.user.status).toBe("invited")

    const approveResponse = await request.post(
      apiUrl(baseUrl, `/members/${member.user.id}/approve`),
      { headers: bearer(adminToken), data: {} },
    )
    const approved = await jsonFromOk<Member>(
      approveResponse,
      "approve target member",
    )
    expect(approved.status).toBe("active")
    expect(approved.must_change_password).toBe(false)
    return member
  })

  const targetContext = await browser.newContext({
    baseURL: baseUrl,
    viewport: { width: 1280, height: 900 },
  })
  const targetPage = await targetContext.newPage()

  try {
    await test.step(
      "User to Contributor is visible after refresh in the same session",
      async () => {
        await login(targetPage, TARGET.email, created.temporary_password)
        await expect(targetPage).toHaveURL(/\/app\/resources(?:\/.*)?$/)
        const originalToken = await sessionToken(targetPage)

        const sidebar = targetPage.locator("aside")
        await expect(
          sidebar.getByRole("link", { name: "Members", exact: true }),
        ).toHaveCount(0)

        await updateRole(
          request,
          baseUrl,
          adminToken,
          created.user.id,
          "contribute",
        )
        const allowedDashboard = await request.get(apiUrl(baseUrl, "/dashboard"), {
          headers: bearer(originalToken),
        })
        expect(allowedDashboard.status()).toBe(200)
        await targetPage.reload()

        await expect(
          sidebar.getByRole("link", { name: "Members", exact: true }),
        ).toBeVisible()
        await expect(
          sidebar.getByRole("link", { name: "Overview", exact: true }),
        ).toBeVisible()
        await expect(
          targetPage
            .locator("header")
            .getByText("Contributor", { exact: true }),
        ).toBeVisible()
        expect(await sessionToken(targetPage)).toBe(originalToken)
      },
    )

    await test.step(
      "Contributor to User denies the next privileged request and removes UI",
      async () => {
        const originalToken = await sessionToken(targetPage)
        await updateRole(request, baseUrl, adminToken, created.user.id, "user")

        const denied = await request.get(apiUrl(baseUrl, "/dashboard"), {
          headers: bearer(originalToken),
        })
        expect(denied.status()).toBe(403)
        await expect(denied.json()).resolves.toMatchObject({
          error_code: "permission_denied",
        })

        await targetPage.reload()
        const sidebar = targetPage.locator("aside")
        await expect(
          sidebar.getByRole("link", { name: "Members", exact: true }),
        ).toHaveCount(0)
        await expect(
          targetPage.locator("header").getByText("User", { exact: true }),
        ).toBeVisible()
        expect(await sessionToken(targetPage)).toBe(originalToken)

        await targetPage.goto("/app/members")
        await expect(
          targetPage.getByRole("heading", { name: "Access unavailable" }),
        ).toBeVisible()
        await expect(
          targetPage.getByText(
            "Forbidden. Ask a project administrator if you need this access.",
          ),
        ).toBeVisible()
      },
    )

    await test.step(
      "Admin elevation invalidates the old session and requires a new token",
      async () => {
        const oldToken = await sessionToken(targetPage)
        await updateRole(request, baseUrl, adminToken, created.user.id, "admin")

        const staleSession = await request.get(apiUrl(baseUrl, "/auth/me"), {
          headers: bearer(oldToken),
        })
        expect(staleSession.status()).toBe(401)
        await expect(staleSession.json()).resolves.toMatchObject({
          error_code: "unauthorized",
        })

        await targetPage.reload()
        await expect(targetPage).toHaveURL(/\/login\?reason=session_expired$/)
        await expect(
          targetPage.getByText(
            "Your session expired or access changed. Sign in again.",
          ),
        ).toBeVisible()
        await expect
          .poll(() =>
            targetPage.evaluate(() =>
              window.sessionStorage.getItem("conductor.token"),
            ),
          )
          .toBeNull()

        await login(targetPage, TARGET.email, created.temporary_password)
        const elevatedToken = await sessionToken(targetPage)
        expect(elevatedToken).not.toBe(oldToken)
        await expect(
          targetPage.locator("header").getByText("Admin", { exact: true }),
        ).toBeVisible()
        await expect(
          targetPage.locator("aside").getByRole("link", {
            name: "Members",
            exact: true,
          }),
        ).toBeVisible()
      },
    )
  } finally {
    await targetContext.close()
  }

  await test.step("authenticated mobile navigation exposes semantic controls", async () => {
    const mobileContext = await browser.newContext({
      baseURL: baseUrl,
      hasTouch: true,
      viewport: { width: 390, height: 844 },
    })
    const mobilePage = await mobileContext.newPage()

    try {
      await login(mobilePage, TARGET.email, created.temporary_password)
      const openMenu = mobilePage.getByRole("button", { name: "Open menu" })
      await expect(openMenu).toBeVisible()
      await openMenu.click()

      const navigationDialog = mobilePage.getByRole("dialog", {
        name: "Project navigation",
      })
      await expect(navigationDialog).toBeVisible()
      await expect
        .poll(async () => Math.round((await navigationDialog.boundingBox())?.x ?? -1))
        .toBe(0)
      await expect(
        navigationDialog.getByRole("link", { name: "Members", exact: true }),
      ).toBeVisible()
      await expect(
        navigationDialog.getByRole("button", { name: "Close menu" }),
      ).toBeVisible()

      const screenshotPath = testInfo.outputPath("authenticated-shell-mobile.png")
      await mobilePage.screenshot({ path: screenshotPath })
      await testInfo.attach("authenticated-shell-mobile", {
        path: screenshotPath,
        contentType: "image/png",
      })
    } finally {
      await mobileContext.close()
    }
  })
})
