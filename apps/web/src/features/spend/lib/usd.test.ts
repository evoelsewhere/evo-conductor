import assert from "node:assert/strict"
import test from "node:test"

import { formatMicrosAsUsd, parseUsdToMicros } from "./usd.ts"

test("dollar amounts become micros without floating-point drift", () => {
  assert.equal(parseUsdToMicros("250"), 250_000_000)
  assert.equal(parseUsdToMicros("12.50"), 12_500_000)
  // 0.07 * 1e6 through a float is 70000.00000000001.
  assert.equal(parseUsdToMicros("0.07"), 70_000)
  assert.equal(parseUsdToMicros(".5"), 500_000)
  assert.equal(parseUsdToMicros("  10  "), 10_000_000)
  assert.equal(parseUsdToMicros("0"), 0)
})

test("anything that is not a plain positive amount is refused", () => {
  for (const input of ["", ".", "-5", "1e6", "12,50", "abc", "$50", "1.2.3"]) {
    assert.equal(parseUsdToMicros(input), null, `${input} should not parse`)
  }
})

// Dropping the seventh decimal would store an allowance nobody asked for.
test("precision finer than a micro-dollar is refused rather than rounded", () => {
  assert.equal(parseUsdToMicros("1.123456"), 1_123_456)
  assert.equal(parseUsdToMicros("1.1234567"), null)
})

test("an amount too large to represent exactly is refused", () => {
  assert.equal(parseUsdToMicros("99999999999999999999"), null)
})

test("micros render back to the amount that produced them", () => {
  for (const input of ["250", "12.50", "0.07", "0"]) {
    const micros = parseUsdToMicros(input)
    assert.notEqual(micros, null)
    assert.equal(parseUsdToMicros(formatMicrosAsUsd(micros as number)), micros)
  }
  assert.equal(formatMicrosAsUsd(250_000_000), "250.00")
  assert.equal(formatMicrosAsUsd(70_000), "0.07")
  assert.equal(formatMicrosAsUsd(1), "0.000001")
})
