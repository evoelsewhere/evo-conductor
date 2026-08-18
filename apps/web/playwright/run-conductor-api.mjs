import { spawn } from "node:child_process"
import { mkdir, rm } from "node:fs/promises"
import { tmpdir } from "node:os"
import path from "node:path"

const RUNTIME_DIRECTORY_ENV = "EVO_CONDUCTOR_PLAYWRIGHT_RUN_DIR"
const RUNTIME_DIRECTORY_PATTERN =
  /^evo-conductor-playwright-\d+-[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/

const configuredDirectory = process.env[RUNTIME_DIRECTORY_ENV]
if (!configuredDirectory) {
  throw new Error(`${RUNTIME_DIRECTORY_ENV} is required`)
}

const runtimeDirectory = path.resolve(configuredDirectory)
const temporaryRoot = path.resolve(tmpdir())
const isDirectTemporaryChild = path.dirname(runtimeDirectory) === temporaryRoot
const isOwnedDirectory = RUNTIME_DIRECTORY_PATTERN.test(
  path.basename(runtimeDirectory),
)

if (!isDirectTemporaryChild || !isOwnedDirectory) {
  throw new Error(
    `Refusing to use unowned Playwright runtime directory: ${runtimeDirectory}`,
  )
}

await mkdir(runtimeDirectory, { mode: 0o700 })

const server = spawn(
  "cargo",
  ["run", "-p", "conductor-server", "--bin", "evo-conductor"],
  {
    cwd: process.cwd(),
    env: process.env,
    stdio: "inherit",
  },
)

let stopping = false

function forwardSignal(signal) {
  stopping = true
  if (server.exitCode === null && server.signalCode === null) {
    server.kill(signal)
  }
}

const stopWithSigint = () => forwardSignal("SIGINT")
const stopWithSigterm = () => forwardSignal("SIGTERM")
process.once("SIGINT", stopWithSigint)
process.once("SIGTERM", stopWithSigterm)

try {
  const result = await new Promise((resolve, reject) => {
    server.once("error", reject)
    server.once("exit", (code, signal) => resolve({ code, signal }))
  })

  if (result.code !== null) {
    process.exitCode = result.code
  } else if (!stopping) {
    console.error(`conductor-server exited from ${result.signal ?? "unknown signal"}`)
    process.exitCode = 1
  }
} finally {
  process.removeListener("SIGINT", stopWithSigint)
  process.removeListener("SIGTERM", stopWithSigterm)
  await rm(runtimeDirectory, { recursive: true, force: true })
}
