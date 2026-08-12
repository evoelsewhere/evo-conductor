import { describe, expect, test } from "bun:test";

import { deterministicUuid, isLocalTarget, normalizeApiPath, normalizeBaseUrl, parseConfig, summarizeHttp, vietnameseMemberDisplayName } from "./fleet-simulator";

describe("fleet simulator safety and determinism", () => {
  test("deterministic UUIDs are stable, distinct and UUID-shaped", () => {
    const first = deterministicUuid("seed:member:1");
    expect(first).toBe(deterministicUuid("seed:member:1"));
    expect(first).not.toBe(deterministicUuid("seed:member:2"));
    expect(first).toMatch(/^[0-9a-f]{8}-[0-9a-f]{4}-5[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/);
  });

  test("normalizes origins and recognizes only explicit loopback hosts", () => {
    expect(normalizeBaseUrl("http://127.0.0.1:4700/api/")).toBe("http://127.0.0.1:4700");
    expect(isLocalTarget("http://localhost:4700")).toBeTrue();
    expect(isLocalTarget("http://127.0.0.1:4700")).toBeTrue();
    expect(isLocalTarget("http://localhost.example.com:4700")).toBeFalse();
    expect(normalizeApiPath("/members")).toBe("/api/members");
    expect(normalizeApiPath("/api/v1/resources/object")).toBe("/api/v1/resources/object");
  });

  test("refuses a remote target without an explicit acknowledgement", () => {
    expect(() => parseConfig(["--base-url", "https://conductor.example.com"], {})).toThrow("refusing non-local target");
    expect(parseConfig(["--base-url", "https://conductor.example.com", "--allow-non-local"], {}).allowNonLocal).toBeTrue();
  });

  test("accepts a session token from the environment without exposing it in CLI options", () => {
    expect(parseConfig([], { FLEET_ADMIN_TOKEN: "local-session" }).adminToken).toBe("local-session");
  });

  test("validates bounded workload controls", () => {
    expect(() => parseConfig(["--members", "0"], {})).toThrow("--members");
    expect(parseConfig(["--requests-per-member", "100"], {}).requestsPerMember).toBe(100);
    expect(() => parseConfig(["--requests-per-member", "501"], {})).toThrow("--requests-per-member");
    expect(() => parseConfig(["--concurrency", "129"], {})).toThrow("--concurrency");
    expect(() => parseConfig(["--history-days", "91"], {})).toThrow("--history-days");
    expect(() => parseConfig(["--member-name-style", "unknown"], {})).toThrow("--member-name-style");
  });

  test("generates 120 unique Vietnamese display names", () => {
    const names = Array.from({ length: 120 }, (_, index) => vietnameseMemberDisplayName(index));
    expect(new Set(names).size).toBe(120);
    expect(names[0]).toMatch(/^[^\x00-\x7F]+|Nguyễn/);
  });

  test("summarizes endpoint latency without exposing request data", () => {
    const summary = summarizeHttp(new Map([["GET /health", { count: 4, retries: 1, failures: 0, latenciesMs: [1, 2, 3, 10] }]]));
    expect(summary["GET /health"]).toEqual({ count: 4, retries: 1, failures: 0, latency_ms: { p50: 2, p95: 10, p99: 10, max: 10 } });
  });
});
