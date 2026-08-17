import { randomUUID } from "node:crypto"
import { tmpdir } from "node:os"
import path from "node:path"
import { fileURLToPath } from "node:url"
import { defineConfig, devices } from "@playwright/test"

const API_PORT = 4711
const WEB_PORT = 5181
const RUNTIME_DIRECTORY_ENV = "EVO_CONDUCTOR_PLAYWRIGHT_RUN_DIR"
const RUNTIME_DIRECTORY_PREFIX = "evo-conductor-playwright-"

const webRoot = path.dirname(fileURLToPath(import.meta.url))
const repositoryRoot = path.resolve(webRoot, "../..")
const runtimeDirectory = path.join(
  tmpdir(),
  `${RUNTIME_DIRECTORY_PREFIX}${process.pid}-${randomUUID()}`,
)

process.env[RUNTIME_DIRECTORY_ENV] = runtimeDirectory

const apiBaseUrl = `http://127.0.0.1:${API_PORT}`
const webBaseUrl = `http://127.0.0.1:${WEB_PORT}`
const databasePath = path.join(runtimeDirectory, "conductor.db")

export default defineConfig({
  testDir: "./e2e",
  outputDir: "./test-results",
  fullyParallel: false,
  workers: 1,
  retries: 0,
  timeout: 45_000,
  expect: {
    timeout: 10_000,
  },
  forbidOnly: Boolean(process.env.CI),
  reporter: [
    ["list"],
    ["html", { outputFolder: "playwright-report", open: "never" }],
  ],
  use: {
    ...devices["Desktop Chrome"],
    baseURL: webBaseUrl,
    screenshot: "only-on-failure",
    trace: "retain-on-failure",
    video: "off",
  },
  webServer: [
    {
      name: "conductor-api",
      command: "exec node apps/web/playwright/run-conductor-api.mjs",
      cwd: repositoryRoot,
      env: {
        CONDUCTOR_DATABASE_URL: `sqlite:${databasePath}?mode=rwc`,
        CONDUCTOR_DATA_DIR: path.join(runtimeDirectory, "data"),
        CONDUCTOR_HOST: "127.0.0.1",
        CONDUCTOR_PORT: String(API_PORT),
        CONDUCTOR_PUBLIC_URL: webBaseUrl,
        RUST_LOG: "evo_conductor=warn,tower_http=warn",
      },
      url: `${apiBaseUrl}/api/health`,
      reuseExistingServer: false,
      timeout: 120_000,
      gracefulShutdown: {
        signal: "SIGTERM",
        timeout: 5_000,
      },
    },
    {
      name: "conductor-web",
      command: `exec bun run dev -- --host 127.0.0.1 --port ${WEB_PORT} --strictPort`,
      cwd: webRoot,
      env: {
        CONDUCTOR_PROXY_TARGET: apiBaseUrl,
      },
      url: webBaseUrl,
      reuseExistingServer: false,
      timeout: 60_000,
      gracefulShutdown: {
        signal: "SIGTERM",
        timeout: 5_000,
      },
    },
  ],
  projects: [
    {
      name: "chromium",
      use: { ...devices["Desktop Chrome"] },
    },
  ],
})
