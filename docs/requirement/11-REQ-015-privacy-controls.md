# REQ-015 — Privacy controls and collection levels

| | |
|---|---|
| ID | REQ-015 |
| Created | 2026-08-09 |
| Updated | 2026-08-14 |
| Status | Draft — collection policy and privacy-safe telemetry boundary partially implemented |
| Priority | P0 |
| Build order | Step 11 of 23 |
| Spec section | [requirements.md section 10](../requirements.md) |
| Source | Baseline specification section 10, extended 2026-08-09 |
| Depends on | REQ-004 |
| Blocks | REQ-012, REQ-013, REQ-014, REQ-016 |
| Repositories | `evo-conductor` and `evoflux` |
| Design | Not created; requires acceptance |

## 1. Context

EvoFlux is positioned as a local-first product. The moment usage data flows to a server, what preserves
user trust is that members know exactly what was sent.

This is a product risk rather than a technical one. A monitoring system that members do not trust will be
worked around rather than used honestly, and the resulting data becomes worthless for the decisions it
was collected to support.

The existing `TelemetrySnapshot` type is already content-free, which is the correct starting point and
should be preserved as the schema grows.

## 2. Requirement

Conductor shall define, publish and enforce a project collection level. No supported level uploads prompt
or response content, reasoning text, source code, terminal output, tool arguments/results, document
content, environment values, credentials or absolute local paths. Collection levels control whether
privacy-safe operational metadata is disabled, counted or enriched with managed-resource attribution.

## 3. The collection level decision

The implementation deliberately does not use a level to authorize work-content capture. The three values
govern progressively richer content-free telemetry:

| Level | Collected | Answers "used for what" | Cost |
|---|---|---|---|
| L0 | No telemetry upload | No | Strongest privacy boundary; inventory/heartbeat may still operate |
| L1 | Content-free request/model/tool counters, timing, provider/model, token and sanitized outcome metadata | Operational usage patterns | Default; excludes managed-resource attribution that is not required for basic counters |
| L2 | L1 plus the richer privacy-safe managed Agent/Skill/Plugin attribution contract | Which governed resource/version contributed | Still excludes prompt/response/reasoning and tool argument/result content |

L1 is the schema and runtime default. An Admin may select L0/L1/L2 in Project Settings; registration
returns the active value and the server refuses telemetry when L0 is active.

## 4. Implementation status

| Implemented | Missing |
|---|---|
| Project Settings stores L0/L1/L2, exposes an Admin-only update API/UI and returns the active policy during registration | L1-versus-L2 field-level enforcement is not yet differentiated in the EvoFlux hook; both currently use the same privacy-safe allowlist |
| L0 blocks server ingestion and stops EvoFlux telemetry flushing | Changing the policy is not audit logged because REQ-018 remains open |
| EvoFlux uses typed field allowlists/redaction and tests that prompts, responses, reasoning, paths, arguments and results never enter the wire payload | A complete normalized-workspace identifier contract and inventory schema proof |
| Personal member usage/activity/tool/request views expose the same privacy-safe event fields through self-or-privileged authorization | Automated administrator-versus-self field-set subset proof and an explicit all-member policy disclosure page |
| Project Settings explains that Conductor never requests work content through this policy | Retention controls under REQ-019 |

### Acceptance progress

| AC | State |
|---|---|
| AC-1–AC-3, AC-7, AC-10, AC-11 | Implemented or substantially implemented for the content-free V1 contract |
| AC-4, AC-8, AC-9 | Partial |
| AC-5, AC-6 | Not complete |

## 5. Acceptance criteria

| ID | Criterion |
|---|---|
| AC-1 | The project collection level is stored as configuration and is readable by every member without special permission |
| AC-2 | The active level is returned to EvoFlux at registration, per [REQ-011](12-REQ-011-client-registration.md) AC-5 |
| AC-3 | EvoFlux enforces the level before transmission; content that the level excludes is never placed on the wire |
| AC-4 | Local file paths are normalized to an approved identifier; absolute paths are never transmitted |
| AC-5 | L2 remains content-free. Enabling any future work-content collection mode would require a separate accepted requirement with explicit Admin action, visible disclosure, restricted viewers and retention; it cannot be introduced by reinterpreting L2 |
| AC-6 | Changing the collection level is recorded in the audit log ([REQ-018](05-REQ-018-audit-logging.md)) |
| AC-7 | A member can view a personal page showing exactly the fields an administrator can see about them, no more and no less |
| AC-8 | An automated test asserts that the field set visible to an administrator about a member is a subset of the field set visible to that member about themselves |
| AC-9 | A plain-language explanation of what is and is not collected is available in the console to all members |
| AC-10 | A schema test asserts that no telemetry or inventory field can carry conversation content, file content or credentials at L0 or L1 |
| AC-11 | The personal transparency view shows the same Agent/Skill/Plugin attribution, timestamps, recorded role, request/model/tool outcomes, separated token counts, estimated-cost source and sanitized errors that an Admin can see about that member, while excluding prompt/response/reasoning and tool argument/result content |

## 6. Out of scope

- Allowing individual members to opt out. Collection level is a project policy, not a personal setting.
- Personal data deletion on request. Reconsider at P2 if a legal obligation applies.
- Holding LLM provider credentials, which specification section 10 places outside scope; see
  [REQ-023](23-REQ-023-ai-gateway.md).

## 7. Risks

| # | Risk | Severity | Mitigation |
|---|---|---|---|
| 1 | Usage data is used to evaluate individuals, so the team learns to avoid it and the data loses value | High | AC-7, AC-8 and AC-9 make collection symmetric and visible; ranking members is explicitly out of scope in [REQ-016](17-REQ-016-usage-aggregation-dashboards.md) |
| 2 | Gradual drift from L1 to L2 without an explicit decision | High | AC-5 makes L2 a governed action rather than a configuration flag |
| 3 | Session titles unintentionally contain sensitive context | Medium | Allow members to edit or suppress a title before it is transmitted |
| 4 | The personal view falls behind the administrator view and becomes an empty promise | Medium | AC-8 turns the principle into a test |
| 5 | Detailed resource/request audit quietly expands into collection of work content | High | AC-10 and AC-11 define a metadata allowlist and symmetric view without payloads |

## 8. Open questions

- Should L1 and L2 retain distinct schemas, or should L2 be renamed to make its governed-resource
  attribution purpose clearer? The current wire value is already deployed, so preserve it until a
  versioned client-policy contract can migrate safely.
- Should a normalized workspace identifier be added at L2? Recommendation: only an explicit user label or
  server-issued opaque ID; never repository name or an absolute/local path by default.

## History

| Date | Change | Author |
|---|---|---|
| 2026-08-09 | Created | |
| 2026-08-11 | Extended personal transparency to the member/resource usage-audit fields required by REQ-016 | Codex |
| 2026-08-14 | Replaced the obsolete content-capture interpretation with the implemented L0 off/L1 counters/L2 privacy-safe attribution policy | Codex |
