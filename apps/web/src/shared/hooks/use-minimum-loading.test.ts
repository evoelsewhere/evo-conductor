import assert from "node:assert/strict"
import test from "node:test"

import {
  DEFAULT_MINIMUM_LOADING_MS,
  minimumLoadingRemaining,
} from "./use-minimum-loading.ts"

test("minimum loading defaults to one second", () => {
  assert.equal(DEFAULT_MINIMUM_LOADING_MS, 1_000)
  assert.equal(minimumLoadingRemaining(100, 100), 1_000)
  assert.equal(minimumLoadingRemaining(100, 1_099), 1)
  assert.equal(minimumLoadingRemaining(100, 1_100), 0)
  assert.equal(minimumLoadingRemaining(100, 1_500), 0)
})

test("minimum loading supports focused custom durations", () => {
  assert.equal(minimumLoadingRemaining(50, 250, 500), 300)
  assert.equal(minimumLoadingRemaining(250, 50, 500), 500)
})
