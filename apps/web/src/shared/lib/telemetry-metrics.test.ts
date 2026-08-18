import assert from "node:assert/strict"
import test from "node:test"

import { terminalRequestSuccessRate } from "./telemetry-metrics.ts"

test("terminalRequestSuccessRate uses every terminal request", () => {
  // Eight successes, one error, and one blocked request is 8 / 10, not 8 / 9.
  assert.equal(terminalRequestSuccessRate(8, 10), 80)
  assert.equal(terminalRequestSuccessRate(0, 10), 0)
  assert.equal(terminalRequestSuccessRate(undefined, 10), 0)
  assert.equal(terminalRequestSuccessRate(0, 0), null)
  assert.equal(terminalRequestSuccessRate(undefined, undefined), null)
})
