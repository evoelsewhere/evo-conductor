#!/usr/bin/env bun

/**
 * Deterministic EvoFlux fleet simulator.
 *
 * This intentionally uses only Conductor's public HTTP API. It can therefore
 * exercise the same auth, authorization, idempotency, delivery, inventory and
 * telemetry boundaries as a real EvoFlux client fleet.
 */

import { createHash } from "node:crypto";
import { chmod, mkdir, readFile, rename, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";

export const DEFAULT_SEED = "2026-08-12";
export const DEFAULT_ADMIN_PASSWORD = "LocalFleetOnly!2026";
const STATE_SCHEMA_VERSION = 1;
const DEFAULT_BASE_URL = "http://127.0.0.1:4700";
const DEFAULT_MEMBER_COUNT = 1_000;
const DEFAULT_REQUESTS_PER_MEMBER = 3;
const DEFAULT_CONCURRENCY = 24;
const DEFAULT_NEGATIVE_EVERY = 50;
const MAX_ATTEMPTS = 4;

type JsonObject = Record<string, unknown>;

export type FleetConfig = {
  baseUrl: string;
  memberCount: number;
  requestsPerMember: number;
  concurrency: number;
  negativeEvery: number;
  seed: string;
  memberPrefix: string;
  emailDomain: string;
  adminEmail: string;
  adminPassword: string;
  adminToken?: string;
  projectName: string;
  memberNameStyle: "fleet" | "vietnamese";
  historyDays: number;
  stateFile: string;
  summaryFile?: string;
  allowNonLocal: boolean;
};

type FleetMemberState = {
  email: string;
  memberId: string;
  token?: string;
  installationId?: string;
};

type FleetState = {
  schemaVersion: number;
  baseUrl: string;
  seed: string;
  members: Record<string, FleetMemberState>;
};

type Member = {
  id: string;
  email: string;
  display_name: string;
  status: "pending" | "invited" | "active" | "disabled";
};

type ManagedResource = {
  id: string;
  kind: ResourceKind;
  slug: string;
  status: string;
  draft_revision: number;
};

type ResourceKind = "agent" | "skill" | "plugin";

type ResourceVersion = {
  id: string;
  version: string;
  status: string;
  active_channel?: string | null;
};

type ResourceFixture = {
  id: string;
  versionId: string;
  version: string;
  kind: ResourceKind;
  slug: string;
  sha256?: string;
};

type ResourceFetchEntry = {
  resource_id: string;
  version_id: string;
  kind: ResourceKind;
  slug: string;
  version: string;
  bundle: {
    artifact_sha256: string;
    artifact_size: number;
    artifact_media_type: string;
    tree_sha256: string;
  };
};

type ResourceFetchPlan = {
  commit: { id: string; tree_sha256: string; sequence: number };
  up_to_date: boolean;
  entries: ResourceFetchEntry[];
  tombstones: { resource_id: string }[];
  objects: { artifact_sha256: string; href: string }[];
};

type RequestMetric = {
  count: number;
  retries: number;
  failures: number;
  latenciesMs: number[];
};

type Counters = {
  membersFound: number;
  membersCreated: number;
  membersActivated: number;
  secretsCreated: number;
  secretsReused: number;
  installationsRegistered: number;
  fetchPlansPulled: number;
  fetchUpToDateConfirmed: number;
  artifactsPulled: number;
  inventoryAccepted: number;
  usageAccepted: number;
  usageDuplicates: number;
  usageRejected: number;
  telemetryAccepted: number;
  telemetryDuplicates: number;
  telemetryRejectedRequests: number;
  expectedNegativeTests: number;
  unexpectedErrors: number;
};

type Invariant = { name: string; passed: boolean; actual: unknown; expected: string };

class ApiError extends Error {
  constructor(
    readonly status: number,
    readonly method: string,
    readonly path: string,
    readonly responseBody: string,
  ) {
    super(`${method} ${path} returned ${status}: ${responseBody.slice(0, 300)}`);
  }
}

export function deterministicUuid(namespace: string): string {
  const bytes = Buffer.from(createHash("sha256").update(namespace).digest().subarray(0, 16));
  bytes[6] = (bytes[6]! & 0x0f) | 0x50;
  bytes[8] = (bytes[8]! & 0x3f) | 0x80;
  const hex = bytes.toString("hex");
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
}

export function normalizeBaseUrl(raw: string): string {
  const url = new URL(raw);
  if (!['http:', 'https:'].includes(url.protocol)) {
    throw new Error("--base-url must use http or https");
  }
  if (url.username || url.password || url.search || url.hash) {
    throw new Error("--base-url must not contain credentials, query parameters or a fragment");
  }
  url.pathname = url.pathname.replace(/\/+$/, "").replace(/\/api$/, "") || "/";
  return url.toString().replace(/\/$/, "");
}

export function isLocalTarget(raw: string): boolean {
  const hostname = new URL(raw).hostname.toLowerCase();
  return hostname === "localhost" || hostname === "127.0.0.1" || hostname === "0.0.0.0" || hostname === "[::1]" || hostname === "::1";
}

export function normalizeApiPath(path: string): string {
  return path.startsWith("/api/") ? path : `/api${path}`;
}

function intFlag(value: string, name: string, minimum: number, maximum: number): number {
  const parsed = Number.parseInt(value, 10);
  if (!Number.isSafeInteger(parsed) || parsed < minimum || parsed > maximum) {
    throw new Error(`${name} must be an integer between ${minimum} and ${maximum}`);
  }
  return parsed;
}

function option(args: string[], name: string, fallback?: string): string | undefined {
  const prefix = `${name}=`;
  const inline = args.find((arg) => arg.startsWith(prefix));
  if (inline) return inline.slice(prefix.length);
  const index = args.indexOf(name);
  if (index >= 0) {
    const value = args[index + 1];
    if (!value || value.startsWith("--")) throw new Error(`${name} requires a value`);
    return value;
  }
  return fallback;
}

function hasFlag(args: string[], name: string): boolean {
  return args.includes(name);
}

export function parseConfig(args: string[], env: Record<string, string | undefined> = process.env): FleetConfig {
  const baseUrl = normalizeBaseUrl(option(args, "--base-url", env.FLEET_BASE_URL ?? DEFAULT_BASE_URL)!);
  const seed = option(args, "--seed", env.FLEET_SEED ?? DEFAULT_SEED)!;
  const defaultState = resolve(`.fleet-simulator/${new URL(baseUrl).hostname}-${seed}.json`);
  const memberNameStyle = option(args, "--member-name-style", env.FLEET_MEMBER_NAME_STYLE ?? "fleet");
  if (memberNameStyle !== "fleet" && memberNameStyle !== "vietnamese") {
    throw new Error("--member-name-style must be fleet or vietnamese");
  }
  const config: FleetConfig = {
    baseUrl,
    memberCount: intFlag(option(args, "--members", env.FLEET_MEMBERS ?? `${DEFAULT_MEMBER_COUNT}`)!, "--members", 1, 10_000),
    requestsPerMember: intFlag(option(args, "--requests-per-member", env.FLEET_REQUESTS_PER_MEMBER ?? `${DEFAULT_REQUESTS_PER_MEMBER}`)!, "--requests-per-member", 1, 33),
    concurrency: intFlag(option(args, "--concurrency", env.FLEET_CONCURRENCY ?? `${DEFAULT_CONCURRENCY}`)!, "--concurrency", 1, 128),
    negativeEvery: intFlag(option(args, "--negative-every", env.FLEET_NEGATIVE_EVERY ?? `${DEFAULT_NEGATIVE_EVERY}`)!, "--negative-every", 0, 10_000),
    seed,
    memberPrefix: option(args, "--member-prefix", env.FLEET_MEMBER_PREFIX ?? "fleet-member")!,
    emailDomain: option(args, "--email-domain", env.FLEET_EMAIL_DOMAIN ?? "fleet.invalid")!,
    adminEmail: option(args, "--admin-email", env.FLEET_ADMIN_EMAIL ?? "fleet-admin@fleet.invalid")!,
    adminPassword: option(args, "--admin-password", env.FLEET_ADMIN_PASSWORD ?? DEFAULT_ADMIN_PASSWORD)!,
    adminToken: env.FLEET_ADMIN_TOKEN,
    projectName: option(args, "--project-name", env.FLEET_PROJECT_NAME ?? "evoflux-fleet-lab")!,
    memberNameStyle,
    historyDays: intFlag(option(args, "--history-days", env.FLEET_HISTORY_DAYS ?? "14")!, "--history-days", 1, 90),
    stateFile: resolve(option(args, "--state-file", env.FLEET_STATE_FILE ?? defaultState)!),
    summaryFile: option(args, "--summary-file", env.FLEET_SUMMARY_FILE),
    allowNonLocal: hasFlag(args, "--allow-non-local") || env.FLEET_ALLOW_NON_LOCAL === "true",
  };
  if (!isLocalTarget(config.baseUrl) && !config.allowNonLocal) {
    throw new Error(`refusing non-local target ${config.baseUrl}; pass --allow-non-local only for an explicitly authorized environment`);
  }
  if (!config.adminEmail.includes("@")) throw new Error("--admin-email must be a valid email");
  if (config.adminPassword.length < 12) throw new Error("--admin-password must contain at least 12 characters");
  if (!/^[a-z0-9][a-z0-9-]{0,48}$/i.test(config.memberPrefix)) throw new Error("--member-prefix contains unsupported characters");
  if (!/^[a-z0-9.-]+$/i.test(config.emailDomain)) throw new Error("--email-domain contains unsupported characters");
  return config;
}

function help(): string {
  return `Evo Conductor deterministic fleet simulator

Usage:
  bun run tools/fleet-simulator.ts [options]

Options:
  --base-url URL               Conductor origin (default ${DEFAULT_BASE_URL})
  --members N                  Member/install count (default ${DEFAULT_MEMBER_COUNT})
  --requests-per-member N      Request triplets per member, 1-33 (default ${DEFAULT_REQUESTS_PER_MEMBER})
  --concurrency N              Concurrent member flows (default ${DEFAULT_CONCURRENCY})
  --negative-every N           One rejected telemetry request every N members; 0 disables
  --seed VALUE                 Stable identity/event seed (default ${DEFAULT_SEED})
  --admin-email EMAIL          Existing admin or local setup admin
  --admin-password PASSWORD    Prefer FLEET_ADMIN_PASSWORD to avoid shell history
                               FLEET_ADMIN_TOKEN may supply an existing local session instead
  --project-name NAME          Used only when completing fresh local setup
  --member-name-style STYLE    Display names: fleet or vietnamese (default fleet)
  --history-days N             Spread activity over the last 1-90 days (default 14)
  --state-file PATH            Resumable secret/install state (mode 0600)
  --summary-file PATH          Also write the final JSON report to this path
  --allow-non-local            Required safety acknowledgement for remote targets
  --help                       Show this text
`;
}

class HttpClient {
  private bearer?: string;
  readonly metrics = new Map<string, RequestMetric>();

  constructor(readonly baseUrl: string) {}

  setBearer(token: string | undefined): void {
    this.bearer = token;
  }

  private metric(key: string): RequestMetric {
    const current = this.metrics.get(key) ?? { count: 0, retries: 0, failures: 0, latenciesMs: [] };
    this.metrics.set(key, current);
    return current;
  }

  async request<T>(method: string, path: string, body?: unknown, options: { token?: string; headers?: Record<string, string>; expectError?: number[]; binary?: boolean } = {}): Promise<T> {
    const metric = this.metric(`${method} ${routeKey(path)}`);
    const started = performance.now();
    let lastError: unknown;
    for (let attempt = 1; attempt <= MAX_ATTEMPTS; attempt += 1) {
      try {
        const token = options.token ?? this.bearer;
        const headers: Record<string, string> = { Accept: "application/json", ...options.headers };
        if (token) headers.Authorization = `Bearer ${token}`;
        if (body !== undefined) headers["Content-Type"] = "application/json";
        const apiPath = normalizeApiPath(path);
        const response = await fetch(`${this.baseUrl}${apiPath}`, {
          method,
          headers,
          body: body === undefined ? undefined : JSON.stringify(body),
        });
        const retryable = response.status === 408 || response.status === 425 || response.status === 429 || response.status >= 500;
        if (retryable && attempt < MAX_ATTEMPTS) {
          metric.retries += 1;
          await response.arrayBuffer();
          await backoff(attempt);
          continue;
        }
        if (options.expectError?.includes(response.status)) {
          await response.arrayBuffer();
          metric.count += 1;
          metric.latenciesMs.push(performance.now() - started);
          return response.status as T;
        }
        if (options.expectError && response.ok) {
          metric.failures += 1;
          await response.arrayBuffer();
          throw new Error(`${method} ${path} was accepted but should have returned ${options.expectError.join(" or ")}`);
        }
        if (!response.ok) {
          const text = await response.text();
          metric.failures += 1;
          throw new ApiError(response.status, method, path, text);
        }
        metric.count += 1;
        metric.latenciesMs.push(performance.now() - started);
        if (options.binary) return new Uint8Array(await response.arrayBuffer()) as T;
        const text = await response.text();
        return (text ? JSON.parse(text) : null) as T;
      } catch (error) {
        if (error instanceof ApiError) throw error;
        lastError = error;
        if (attempt < MAX_ATTEMPTS) {
          metric.retries += 1;
          await backoff(attempt);
          continue;
        }
      }
    }
    metric.failures += 1;
    throw new Error(`${method} ${path} failed after ${MAX_ATTEMPTS} attempts: ${String(lastError)}`);
  }
}

function routeKey(path: string): string {
  return path
    .replace(/[0-9a-f]{8}-[0-9a-f-]{27,}/gi, ":id")
    .replace(/cursor=[^&]+/, "cursor=:cursor");
}

function backoff(attempt: number): Promise<void> {
  const delayMs = 50 * (2 ** (attempt - 1));
  return new Promise((resolveDelay) => setTimeout(resolveDelay, delayMs));
}

async function loadState(config: FleetConfig): Promise<FleetState> {
  try {
    const state = JSON.parse(await readFile(config.stateFile, "utf8")) as FleetState;
    if (state.schemaVersion !== STATE_SCHEMA_VERSION || state.baseUrl !== config.baseUrl || state.seed !== config.seed) {
      throw new Error("state file does not match this simulator version, base URL and seed");
    }
    return state;
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error;
    return { schemaVersion: STATE_SCHEMA_VERSION, baseUrl: config.baseUrl, seed: config.seed, members: {} };
  }
}

async function saveJsonSecure(path: string, value: unknown): Promise<void> {
  await mkdir(dirname(path), { recursive: true });
  const temporary = `${path}.tmp`;
  await writeFile(temporary, `${JSON.stringify(value, null, 2)}\n`, { mode: 0o600 });
  await chmod(temporary, 0o600);
  await rename(temporary, path);
}

function memberEmail(config: FleetConfig, index: number): string {
  return `${config.memberPrefix}-${String(index + 1).padStart(4, "0")}-${config.seed.replace(/[^a-z0-9]/gi, "").toLowerCase()}@${config.emailDomain}`;
}

const VIETNAMESE_SURNAMES = [
  "Nguyễn", "Trần", "Lê", "Phạm", "Hoàng", "Huỳnh", "Phan", "Vũ", "Võ", "Đặng", "Bùi", "Đỗ", "Hồ", "Ngô", "Dương",
] as const;

const VIETNAMESE_MIDDLE_NAMES = [
  "Văn", "Thị", "Minh", "Ngọc", "Quang", "Thanh", "Đức", "Gia", "Hải", "Khánh", "Tuấn", "Phương",
] as const;

const VIETNAMESE_GIVEN_NAMES = [
  "An", "Anh", "Bảo", "Bình", "Châu", "Chi", "Cường", "Dũng", "Duy", "Giang", "Hà", "Hạnh", "Hiếu", "Hòa", "Hùng",
  "Huy", "Khải", "Khoa", "Lan", "Linh", "Long", "Mai", "Nam", "Nga", "Ngân", "Nhung", "Phong", "Phúc", "Quân", "Quỳnh",
  "Sơn", "Tâm", "Thảo", "Trang", "Trung", "Tú", "Uyên", "Việt", "Vy", "Yến",
] as const;

export function vietnameseMemberDisplayName(index: number): string {
  const surname = VIETNAMESE_SURNAMES[index % VIETNAMESE_SURNAMES.length]!;
  const middle = VIETNAMESE_MIDDLE_NAMES[Math.floor(index / VIETNAMESE_SURNAMES.length) % VIETNAMESE_MIDDLE_NAMES.length]!;
  const given = VIETNAMESE_GIVEN_NAMES[(index * 7 + Math.floor(index / VIETNAMESE_SURNAMES.length)) % VIETNAMESE_GIVEN_NAMES.length]!;
  return `${surname} ${middle} ${given}`;
}

function memberDisplayName(config: FleetConfig, index: number): string {
  return config.memberNameStyle === "vietnamese"
    ? vietnameseMemberDisplayName(index)
    : `Fleet Member ${String(index + 1).padStart(4, "0")}`;
}

function historicalTimestamp(config: FleetConfig, index: number, requestIndex = 0): string {
  const ordinal = index * config.requestsPerMember + requestIndex;
  const dayOffset = ordinal % config.historyDays;
  const minuteOffset = (ordinal * 47) % (24 * 60);
  return new Date(Date.now() - dayOffset * 86_400_000 - minuteOffset * 60_000).toISOString();
}

async function setupAndLogin(client: HttpClient, config: FleetConfig): Promise<string> {
  if (config.adminToken) {
    client.setBearer(config.adminToken);
    const current = await client.request<{ primary_role: string }>("GET", "/auth/me");
    if (current.primary_role !== "admin") throw new Error("FLEET_ADMIN_TOKEN must belong to an administrator");
    return config.adminToken;
  }
  const setup = await client.request<{ configured: boolean }>("GET", "/setup/status");
  if (!setup.configured) {
    const port = Number.parseInt(new URL(config.baseUrl).port || (new URL(config.baseUrl).protocol === "https:" ? "443" : "80"), 10);
    await client.request("POST", "/setup", {
      project_name: config.projectName,
      display_name: "EvoFlux Fleet Lab",
      bind_host: new URL(config.baseUrl).hostname,
      bind_port: port,
      public_url: config.baseUrl,
      admin_email: config.adminEmail,
      admin_display_name: "Fleet Administrator",
      admin_password: config.adminPassword,
      sso: null,
    });
  }
  const session = await client.request<{ token: string }>("POST", "/auth/login", {
    email: config.adminEmail,
    password: config.adminPassword,
  });
  client.setBearer(session.token);
  return session.token;
}

async function ensureResources(client: HttpClient): Promise<ResourceFixture[]> {
  const definitions: Array<{ kind: ResourceKind; slug: string; name: string }> = [
    { kind: "agent", slug: "fleet-reviewer", name: "Fleet Reviewer" },
    { kind: "skill", slug: "fleet-release-audit", name: "Fleet Release Audit" },
    { kind: "plugin", slug: "fleet-toolkit", name: "Fleet Toolkit" },
  ];
  let resources = await client.request<ManagedResource[]>("GET", "/resources");
  const fixtures: ResourceFixture[] = [];
  for (const definition of definitions) {
    let resource = resources.find((item) => item.kind === definition.kind && item.slug === definition.slug);
    if (!resource) {
      resource = await client.request<ManagedResource>("POST", "/resources", {
        kind: definition.kind,
        slug: definition.slug,
        name: definition.name,
        description: "Deterministic fleet simulator fixture",
        version: "0.1.0",
        visibility: "shared",
        payload: {},
        changelog: "Create deterministic fleet fixture",
      });
      resources = [...resources, resource];
    }
    let versions = await client.request<ResourceVersion[]>("GET", `/resources/${resource.id}/versions`);
    let active = versions.find((version) => version.active_channel === "published")
      ?? versions.find((version) => version.status === "published");
    if (!active) {
      const tree = await client.request<{ revision: number }>("GET", `/resources/${resource.id}/draft/files`);
      const validation = await client.request<{ valid: boolean; diagnostics: unknown[] }>("POST", `/resources/${resource.id}/draft/validate`, {});
      if (!validation.valid) throw new Error(`fleet ${definition.kind} fixture failed validation: ${JSON.stringify(validation.diagnostics)}`);
      const released = await client.request<{ version_id: string; version: string }>("POST", `/resources/${resource.id}/release`, {
        channel: "published",
        version_mode: "auto",
        manual_version: null,
        draft_revision: tree.revision,
        changelog: "Publish deterministic fleet fixture",
        beta_member_ids: [],
        minimum_evoflux_version: "0.1.0",
      });
      active = { id: released.version_id, version: released.version, status: "published", active_channel: "published" };
    }
    fixtures.push({ id: resource.id, versionId: active.id, version: active.version, kind: definition.kind, slug: definition.slug });
  }
  return fixtures;
}

async function listMembers(client: HttpClient): Promise<Map<string, Member>> {
  const members = new Map<string, Member>();
  let page = 1;
  for (;;) {
    const response = await client.request<{ items: Member[]; total: number; limit: number }>("GET", `/members?page=${page}&limit=100`);
    for (const item of response.items) members.set(item.email.toLowerCase(), item);
    if (members.size >= response.total || response.items.length === 0) break;
    page += 1;
  }
  return members;
}

async function activateMember(client: HttpClient, member: Member, counters: Counters): Promise<Member> {
  if (member.status === "active") return member;
  if (member.status === "disabled") {
    const enabled = await client.request<Member>("POST", `/members/${member.id}/enable`, {});
    counters.membersActivated += 1;
    return enabled;
  }
  const approved = await client.request<Member>("POST", `/members/${member.id}/approve`, {});
  counters.membersActivated += 1;
  return approved;
}

async function ensureMember(client: HttpClient, config: FleetConfig, index: number, existing: Map<string, Member>, counters: Counters): Promise<Member> {
  const email = memberEmail(config, index);
  let member = existing.get(email);
  if (!member) {
    const created = await client.request<{ user: Member }>("POST", "/members", {
      email,
      display_name: memberDisplayName(config, index),
      primary_role: "user",
      sub_role_ids: [],
      tag_ids: [],
    });
    member = created.user;
    existing.set(email, member);
    counters.membersCreated += 1;
  } else {
    counters.membersFound += 1;
  }
  member = await activateMember(client, member, counters);
  existing.set(email, member);
  return member;
}

async function issueSecret(client: HttpClient, memberId: string, index: number): Promise<string> {
  const created = await client.request<{ token: string }>("POST", `/members/${memberId}/secrets`, {
    name: `Fleet desktop ${String(index + 1).padStart(4, "0")}`,
    scopes: ["subscribe_resources", "report_telemetry", "sync_inventory"],
    expires_at: null,
  });
  return created.token;
}

async function registerInstallation(client: HttpClient, config: FleetConfig, memberId: string, token: string, index: number): Promise<string> {
  const installationKey = deterministicUuid(`${config.seed}:installation:${memberId}`);
  const registrationKey = deterministicUuid(`${config.seed}:registration:${memberId}`);
  const platform = ["macos", "linux", "windows"][index % 3];
  const registered = await client.request<{ installation: { id: string } }>("POST", "/v1/client/register", {
    installation_key: installationKey,
    display_name: `Fleet ${platform} ${String(index + 1).padStart(4, "0")}`,
    platform,
    evoflux_version: `0.9.${index % 7}`,
    workspace_association: `fleet-${String(index + 1).padStart(4, "0")}`,
  }, { token, headers: { "Idempotency-Key": registrationKey } });
  return registered.installation.id;
}

async function smartFetch(client: HttpClient, token: string, installationId: string, haveCommit: string | null, have: { resource_id: string; version_id: string; artifact_sha256: string }[]): Promise<ResourceFetchPlan> {
  return client.request<ResourceFetchPlan>("POST", "/v1/resources/fetch", {
    installation_id: installationId,
    have_commit: haveCommit,
    have,
  }, { token });
}

async function pullResourceObjects(client: HttpClient, token: string, fixtures: ResourceFixture[], plan: ResourceFetchPlan, counters: Counters): Promise<ResourceFixture[]> {
  const relevant = fixtures.map((fixture) => {
    const entry = plan.entries.find((candidate) => candidate.resource_id === fixture.id);
    if (!entry) throw new Error(`resource ${fixture.slug} missing from smart fetch tree`);
    return { ...fixture, versionId: entry.version_id, version: entry.version, sha256: entry.bundle.artifact_sha256 };
  });
  for (const resource of relevant) {
    const object = plan.objects.find((candidate) => candidate.artifact_sha256 === resource.sha256);
    if (!object) throw new Error(`artifact ${resource.sha256} missing from fetch object plan`);
    const bytes = await client.request<Uint8Array>("GET", object.href, undefined, { token, binary: true });
    counters.artifactsPulled += 1;
    if (resource.sha256) {
      const actual = createHash("sha256").update(bytes).digest("hex");
      if (actual !== resource.sha256) throw new Error(`artifact digest mismatch for ${resource.slug}`);
    }
  }
  return relevant;
}

function inventoryState(index: number, resource: ResourceFixture): { state: string; applied: boolean; error: string | null } {
  // Keep one resource applied for every member so installation/member adoption
  // invariants remain true. Agent/Skill rows below still populate the pending
  // and attention segments used by monitoring charts.
  if (resource.kind === "plugin") return { state: "in_sync", applied: true, error: null };
  if (index % 37 === 0) return { state: "error", applied: false, error: "fleet_apply_failure" };
  if (index % 23 === 0) return { state: "update_pending", applied: false, error: null };
  return { state: "in_sync", applied: true, error: null };
}

async function reportInventory(client: HttpClient, config: FleetConfig, token: string, installationId: string, memberId: string, resources: ResourceFixture[], index: number): Promise<number> {
  const observedAt = new Date().toISOString();
  const items = resources.map((resource) => {
    const observation = inventoryState(index, resource);
    return {
      resource_id: resource.id,
      desired_version_id: resource.versionId,
      applied_version_id: observation.applied ? resource.versionId : null,
      release_channel: "published",
      content_sha256: observation.applied ? resource.sha256 ?? null : null,
      plugin_installation_id: resource.kind === "plugin" ? deterministicUuid(`${config.seed}:plugin:${memberId}`) : null,
      observed_state: observation.state,
      error_category: observation.error,
      observed_at: observedAt,
    };
  });
  const response = await client.request<{ accepted: number }>("PUT", "/v1/client/inventory", { installation_id: installationId, items }, { token });
  return response.accepted;
}

function outcome(index: number, requestIndex: number): "success" | "error" | "blocked" | "cancelled" {
  const bucket = (index * 17 + requestIndex * 29) % 100;
  if (bucket < 88) return "success";
  if (bucket < 94) return "error";
  if (bucket < 97) return "blocked";
  return "cancelled";
}

function attributions(config: FleetConfig, memberId: string, resources: ResourceFixture[]) {
  const byKind = Object.fromEntries(resources.map((resource) => [resource.kind, resource])) as Record<ResourceKind, ResourceFixture>;
  const pluginInstallationId = deterministicUuid(`${config.seed}:plugin:${memberId}`);
  return {
    agent: { resource_id: byKind.agent.id, version_id: byKind.agent.versionId, relation: "executing_agent", plugin_installation_id: null },
    skill: { resource_id: byKind.skill.id, version_id: byKind.skill.versionId, relation: "activated_skill", plugin_installation_id: null },
    plugin: { resource_id: byKind.plugin.id, version_id: byKind.plugin.versionId, relation: "plugin_contributed_tool", plugin_installation_id: pluginInstallationId },
  };
}

function telemetryEvents(config: FleetConfig, memberId: string, index: number, resources: ResourceFixture[]): JsonObject[] {
  const refs = attributions(config, memberId, resources);
  const events: JsonObject[] = [];
  for (let requestIndex = 0; requestIndex < config.requestsPerMember; requestIndex += 1) {
    const requestId = `fleet-${config.seed}-${String(index + 1).padStart(4, "0")}-${requestIndex + 1}`;
    const sessionId = `fleet-session-${String(index + 1).padStart(4, "0")}`;
    const status = outcome(index, requestIndex);
    const reportedAt = historicalTimestamp(config, index, requestIndex);
    const common = { request_id: requestId, session_id: sessionId, agent_name: "fleet-reviewer", reported_at: reportedAt, evoflux_version: `0.9.${index % 7}` };
    events.push({
      ...common,
      event_id: deterministicUuid(`${config.seed}:event:${memberId}:${requestIndex}:model`), event_type: "model_call", sequence: 1,
      provider: index % 4 === 0 ? "anthropic" : "openai", model: index % 4 === 0 ? "claude-sonnet-4" : "gpt-5.2", response_model: null,
      tokens_in: 600 + (index % 300), tokens_out: 180 + (requestIndex * 20), cache_read_tokens: index % 2 === 0 ? 120 : 0,
      reasoning_tokens: 40 + (index % 50), tool_use_tokens: 20, duration_ms: 500 + (index % 900), tool_name: null, tool_category: null,
      status: status === "blocked" ? "blocked" : status === "cancelled" ? "cancelled" : status === "error" ? "error" : "success",
      error_category: status === "error" ? "provider_error" : null,
      estimated_cost_usd_micros: 900 + (index % 400), cost_source: "evoflux_catalog", resources: [refs.agent, refs.skill],
    });
    events.push({
      ...common,
      event_id: deterministicUuid(`${config.seed}:event:${memberId}:${requestIndex}:tool`), event_type: "tool_call", sequence: 2,
      provider: null, model: null, response_model: null, tokens_in: 0, tokens_out: 0, cache_read_tokens: 0, reasoning_tokens: 0, tool_use_tokens: 0,
      duration_ms: 80 + (index % 300), tool_name: ["read_file", "search_code", "run_tests"][index % 3],
      tool_category: index % 3 === 2 ? "other" : "filesystem", status: status === "blocked" ? "blocked" : status === "error" ? "error" : "success",
      error_category: status === "error" ? "tool_failure" : null, estimated_cost_usd_micros: null, cost_source: null,
      resources: [refs.agent, refs.skill, refs.plugin],
    });
    events.push({
      ...common,
      event_id: deterministicUuid(`${config.seed}:event:${memberId}:${requestIndex}:request`), event_type: "request", sequence: 3,
      provider: null, model: null, response_model: null, tokens_in: 0, tokens_out: 0, cache_read_tokens: 0, reasoning_tokens: 0, tool_use_tokens: 0,
      duration_ms: 900 + (index % 1_100), tool_name: null, tool_category: null, status,
      error_category: status === "error" ? "terminal_error" : null, estimated_cost_usd_micros: null, cost_source: null,
      resources: [refs.agent, refs.skill],
    });
  }
  return events;
}

async function reportTelemetry(client: HttpClient, config: FleetConfig, token: string, installationId: string, memberId: string, index: number, resources: ResourceFixture[], counters: Counters): Promise<void> {
  const body = { installation_id: installationId, events: telemetryEvents(config, memberId, index, resources) };
  const first = await client.request<{ accepted: number; duplicates: number }>("POST", "/v1/telemetry/batch", body, { token });
  counters.telemetryAccepted += first.accepted;
  counters.telemetryDuplicates += first.duplicates;
  const replay = await client.request<{ accepted: number; duplicates: number }>("POST", "/v1/telemetry/batch", body, { token });
  counters.telemetryAccepted += replay.accepted;
  counters.telemetryDuplicates += replay.duplicates;

  if (config.negativeEvery > 0 && index % config.negativeEvery === 0) {
    counters.expectedNegativeTests += 1;
    const invalid = {
      installation_id: installationId,
      events: [{
        event_id: deterministicUuid(`${config.seed}:invalid:${memberId}`), request_id: `fleet-invalid-${index + 1}`,
        session_id: null, event_type: "tool_call", sequence: 1, agent_name: "fleet-reviewer", provider: null, model: null,
        response_model: null, tokens_in: 0, tokens_out: 0, cache_read_tokens: 0, reasoning_tokens: 0, tool_use_tokens: 0,
        duration_ms: 10, tool_name: null, tool_category: "filesystem", status: "success", error_category: null,
        estimated_cost_usd_micros: null, cost_source: null, evoflux_version: "0.9.0", resources: [], reported_at: new Date().toISOString(),
      }],
    };
    await client.request<number>("POST", "/v1/telemetry/batch", invalid, { token, expectError: [400, 422] });
    counters.telemetryRejectedRequests += 1;
  }
}

async function reportLegacyUsage(client: HttpClient, config: FleetConfig, token: string, memberId: string, index: number, resources: ResourceFixture[], counters: Counters): Promise<void> {
  const resource = resources[index % resources.length]!;
  const validId = deterministicUuid(`${config.seed}:usage:${memberId}`);
  const valid = {
    event_id: validId, resource_id: resource.id, resource_version: resource.version, session_id: `fleet-usage-${index + 1}`,
    outcome: index % 11 === 0 ? "failure" : "success", duration_ms: 300 + (index % 700), tokens_in: 250 + (index % 100), tokens_out: 80,
    occurred_at: historicalTimestamp(config, index),
  };
  const response = await client.request<{ accepted: number; duplicates: number; rejected: number }>("POST", "/v1/usage/resources", {
    events: [valid, valid, { ...valid, event_id: deterministicUuid(`${config.seed}:usage-invalid:${memberId}`), occurred_at: "2000-01-01T00:00:00.000Z" }],
  }, { token });
  counters.usageAccepted += response.accepted;
  counters.usageDuplicates += response.duplicates;
  counters.usageRejected += response.rejected;
}

async function heartbeat(client: HttpClient, token: string, installationId: string): Promise<void> {
  await client.request("POST", "/v1/client/heartbeat", { installation_id: installationId }, { token });
}

async function inChunks<T>(items: T[], size: number, operation: (item: T) => Promise<void>, checkpoint: () => Promise<void>): Promise<void> {
  for (let start = 0; start < items.length; start += size) {
    await Promise.all(items.slice(start, start + size).map(operation));
    await checkpoint();
    console.error(`[fleet] completed ${Math.min(start + size, items.length)}/${items.length} member flows`);
  }
}

function emptyCounters(): Counters {
  return {
    membersFound: 0, membersCreated: 0, membersActivated: 0, secretsCreated: 0, secretsReused: 0,
    installationsRegistered: 0, fetchPlansPulled: 0, fetchUpToDateConfirmed: 0, artifactsPulled: 0,
    inventoryAccepted: 0, usageAccepted: 0, usageDuplicates: 0, usageRejected: 0,
    telemetryAccepted: 0, telemetryDuplicates: 0, telemetryRejectedRequests: 0,
    expectedNegativeTests: 0, unexpectedErrors: 0,
  };
}

function percentile(values: number[], ratio: number): number {
  if (values.length === 0) return 0;
  const sorted = [...values].sort((a, b) => a - b);
  return sorted[Math.min(sorted.length - 1, Math.ceil(sorted.length * ratio) - 1)]!;
}

export function summarizeHttp(metrics: Map<string, RequestMetric>) {
  return Object.fromEntries([...metrics.entries()].sort(([left], [right]) => left.localeCompare(right)).map(([route, metric]) => [route, {
    count: metric.count,
    retries: metric.retries,
    failures: metric.failures,
    latency_ms: {
      p50: Number(percentile(metric.latenciesMs, 0.5).toFixed(2)),
      p95: Number(percentile(metric.latenciesMs, 0.95).toFixed(2)),
      p99: Number(percentile(metric.latenciesMs, 0.99).toFixed(2)),
      max: Number(Math.max(0, ...metric.latenciesMs).toFixed(2)),
    },
  }]));
}

async function run(config: FleetConfig): Promise<JsonObject> {
  const startedAt = new Date();
  const started = performance.now();
  const client = new HttpClient(config.baseUrl);
  const counters = emptyCounters();
  const state = await loadState(config);
  console.error(`[fleet] target=${config.baseUrl} members=${config.memberCount} concurrency=${config.concurrency} seed=${config.seed}`);
  await setupAndLogin(client, config);
  const fixtures = await ensureResources(client);
  const existing = await listMembers(client);
  const indexes = Array.from({ length: config.memberCount }, (_, index) => index);

  await inChunks(indexes, config.concurrency, async (index) => {
    try {
      const member = await ensureMember(client, config, index, existing, counters);
      const email = memberEmail(config, index);
      const memberState = state.members[email] ?? { email, memberId: member.id };
      memberState.memberId = member.id;
      let token = memberState.token;
      if (!token) {
        token = await issueSecret(client, member.id, index);
        memberState.token = token;
        counters.secretsCreated += 1;
      } else {
        counters.secretsReused += 1;
      }
      let installationId: string;
      try {
        installationId = await registerInstallation(client, config, member.id, token, index);
      } catch (error) {
        if (!(error instanceof ApiError) || error.status !== 401) throw error;
        token = await issueSecret(client, member.id, index);
        memberState.token = token;
        counters.secretsCreated += 1;
        installationId = await registerInstallation(client, config, member.id, token, index);
      }
      memberState.installationId = installationId;
      state.members[email] = memberState;
      counters.installationsRegistered += 1;
      const plan = await smartFetch(client, token, installationId, null, []);
      counters.fetchPlansPulled += 1;
      const resources = await pullResourceObjects(client, token, fixtures, plan, counters);
      const repeated = await smartFetch(client, token, installationId, plan.commit.id, plan.entries.map((entry) => ({
        resource_id: entry.resource_id,
        version_id: entry.version_id,
        artifact_sha256: entry.bundle.artifact_sha256,
      })));
      counters.fetchPlansPulled += 1;
      if (!repeated.up_to_date || repeated.entries.length > 0 || repeated.objects.length > 0) {
        throw new Error("smart fetch did not confirm an unchanged checkout");
      }
      counters.fetchUpToDateConfirmed += 1;
      counters.inventoryAccepted += await reportInventory(client, config, token, installationId, member.id, resources, index);
      await reportLegacyUsage(client, config, token, member.id, index, resources, counters);
      await reportTelemetry(client, config, token, installationId, member.id, index, resources, counters);
      await heartbeat(client, token, installationId);
    } catch (error) {
      counters.unexpectedErrors += 1;
      console.error(`[fleet] member ${index + 1} failed: ${String(error)}`);
      throw error;
    }
  }, () => saveJsonSecure(config.stateFile, state));

  const analytics = await client.request<{ totals: Record<string, number>; members: unknown[]; resources: unknown[] }>("GET", "/analytics/resource-usage?limit=100");
  const memberList = await client.request<{ total: number }>("GET", "/members?page=1&limit=1");
  const expectedTelemetryEvents = config.memberCount * config.requestsPerMember * 3;
  const expectedNegative = config.negativeEvery === 0 ? 0 : Math.floor((config.memberCount - 1) / config.negativeEvery) + 1;
  const invariants: Invariant[] = [
    { name: "fleet members exist", passed: memberList.total >= config.memberCount + 1, actual: memberList.total, expected: `>= ${config.memberCount + 1}` },
    { name: "all installations registered", passed: counters.installationsRegistered === config.memberCount, actual: counters.installationsRegistered, expected: `${config.memberCount}` },
    { name: "all smart fetch checkouts confirmed", passed: counters.fetchUpToDateConfirmed === config.memberCount, actual: counters.fetchUpToDateConfirmed, expected: `${config.memberCount}` },
    // Inventory is an upsert. On an idempotent rerun the API may report only
    // changed rows, so the authoritative invariant is the analytics snapshot.
    { name: "all resources inventoried", passed: (analytics.totals.reported_installations ?? 0) >= config.memberCount * fixtures.length, actual: analytics.totals.reported_installations, expected: `>= ${config.memberCount * fixtures.length}` },
    { name: "telemetry first-send or replay accounted", passed: counters.telemetryAccepted + counters.telemetryDuplicates === expectedTelemetryEvents * 2, actual: counters.telemetryAccepted + counters.telemetryDuplicates, expected: `${expectedTelemetryEvents * 2}` },
    { name: "negative telemetry rejected", passed: counters.telemetryRejectedRequests === expectedNegative, actual: counters.telemetryRejectedRequests, expected: `${expectedNegative}` },
    { name: "usage outcomes accounted", passed: counters.usageAccepted + counters.usageDuplicates + counters.usageRejected === config.memberCount * 3, actual: counters.usageAccepted + counters.usageDuplicates + counters.usageRejected, expected: `${config.memberCount * 3}` },
    { name: "analytics sees fleet requests", passed: (analytics.totals.requests ?? 0) >= config.memberCount * config.requestsPerMember, actual: analytics.totals.requests, expected: `>= ${config.memberCount * config.requestsPerMember}` },
    { name: "analytics sees installed members", passed: (analytics.totals.installed_members ?? 0) >= config.memberCount, actual: analytics.totals.installed_members, expected: `>= ${config.memberCount}` },
    { name: "no unexpected errors", passed: counters.unexpectedErrors === 0, actual: counters.unexpectedErrors, expected: "0" },
  ];
  const durationMs = performance.now() - started;
  const http = summarizeHttp(client.metrics);
  const httpRetries = Object.values(http).reduce((total, item) => total + item.retries, 0);
  const summary: JsonObject = {
    schema_version: 1,
    run: {
      seed: config.seed,
      base_url: config.baseUrl,
      member_count: config.memberCount,
      requests_per_member: config.requestsPerMember,
      concurrency: config.concurrency,
      member_name_style: config.memberNameStyle,
      history_days: config.historyDays,
      started_at: startedAt.toISOString(),
      finished_at: new Date().toISOString(),
      duration_ms: Number(durationMs.toFixed(2)),
      members_per_second: Number((config.memberCount / (durationMs / 1_000)).toFixed(2)),
      telemetry_events_per_second: Number((expectedTelemetryEvents / (durationMs / 1_000)).toFixed(2)),
      state_file: config.stateFile,
    },
    counters: { ...counters, httpRetries },
    analytics_totals: analytics.totals,
    invariants: {
      passed: invariants.filter((item) => item.passed).length,
      failed: invariants.filter((item) => !item.passed).length,
      checks: invariants,
    },
    http,
  };
  if (config.summaryFile) await saveJsonSecure(resolve(config.summaryFile), summary);
  return summary;
}

export async function main(args = process.argv.slice(2)): Promise<void> {
  if (hasFlag(args, "--help")) {
    console.log(help());
    return;
  }
  const config = parseConfig(args);
  const summary = await run(config);
  console.log(JSON.stringify(summary, null, 2));
  const invariants = summary.invariants as { failed: number };
  if (invariants.failed > 0) process.exitCode = 2;
}

if (import.meta.main) {
  main().catch((error) => {
    console.error(`[fleet] fatal: ${error instanceof Error ? error.message : String(error)}`);
    process.exitCode = 1;
  });
}
