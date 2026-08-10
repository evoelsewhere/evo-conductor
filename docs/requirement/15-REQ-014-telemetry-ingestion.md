# REQ-014 — Telemetry ingestion

| | |
|---|---|
| ID | REQ-014 |
| Created | 2026-08-09 |
| Updated | 2026-08-11 |
| Status | Accepted (2026-08-10; project-owner usage-audit extension 2026-08-11) — partial implementation in review |
| Priority | P0 |
| Build order | Step 15 of 23 |
| Spec section | [requirements.md section 9](../requirements.md) |
| Source | Baseline specification section 9; project-owner member/resource usage-audit extension 2026-08-11 |
| Depends on | REQ-001, REQ-011, REQ-015 |
| Blocks | REQ-016, REQ-017, V1 acceptance criterion 9 |
| Repositories | `evo-conductor` and `evoflux` |
| Design | Not created; requirement accepted 2026-08-10, so an as-built reconciliation design is the next lifecycle artifact |

## 1. Context

Monitoring is the capability the project owner has identified as most important. The code now has
partial generic telemetry and catalog-resource usage paths, but neither can yet answer the complete
member/resource audit question and using both independently would double-count the same work.

Two properties of the environment shape the design and are easy to overlook. EvoFlux is a local-first
desktop application that will regularly be offline, and its clock cannot be trusted.

## 2. Requirement

EvoFlux shall send batched usage telemetry to Conductor. Ingestion shall be idempotent, shall tolerate
replay after network interruption, and shall record both client-reported and server-assigned timestamps.
The contract shall let an authorized viewer answer, without reading work content: which member used which
project-owned Agent, Skill or Plugin version; when; from which installation; through which model/tool;
with what success/error result, duration, token consumption, model-call count and estimated cost.

### 2.1 Event grain, correlation and resource attribution

One user request may execute several agents, activate several Skills, call a model several times because
of tools or retries, and invoke tools contributed by a Plugin. Telemetry shall preserve this hierarchy:

- `request_id` identifies one user-initiated request/turn and has one terminal request outcome.
- `run_id` identifies an Agent run within the request.
- each model call and tool call has its own event ID, sequence, status and duration;
- `parent_event_id` or equivalent correlation links child calls without transmitting messages;
- every Conductor-managed resource reference carries `project_id`, `resource_id`, immutable `version_id`,
  kind (`agent`, `skill` or `plugin`) and relation (`executing_agent`, `activated_skill`,
  `plugin_contributed_skill` or `plugin_contributed_tool`);
- Plugin attribution comes from the managed installation/resource mapping, never from guessing a tool
  name, and standalone Skills remain distinguishable from Skills bundled by a Plugin.

Project and member identity are derived from the authenticated token. EvoFlux supplies only the stable
installation and managed-resource references it previously received from Conductor; Conductor rejects a
cross-project, inaccessible or unknown resource/version reference. Conductor stamps the member's
server-owned primary role, sub-role IDs and tag IDs at ingestion so role analysis never trusts a role
claimed by the client. The UI shall label this dimension as the role recorded at ingestion when an
offline event's client timestamp predates receipt.

Metric grain is fixed to prevent double counting: requests are distinct `request_id`; resource uses are
distinct `(request_id, resource_id, version_id, relation)`; model calls count model-call events including
retries; token and cost totals sum model-call events only; request success/error uses the terminal request
outcome; tool success/error uses tool-call outcomes. Project totals count each fact once. Resource
breakdowns may overlap when one request legitimately uses multiple resources and must be labelled as
attributed usage rather than added together as a new project total.

## 3. Implementation status

The project owner accepted this requirement on 2026-08-10. Partial implementation is open in
[evo-conductor#2](https://github.com/evoelsewhere/evo-conductor/pull/2) and
[evoflux#4](https://github.com/evoelsewhere/evoflux/pull/4).

| Delivered in review | Evidence |
|---|---|
| Typed allowlisted model/tool events, token counters, duration and sanitized status/error metadata | Conductor commits [`b7d6f93`](https://github.com/evoelsewhere/evo-conductor/commit/b7d6f937b078ad24fb152d6e0b1f3b8175b08aa3) and [`6149a2c`](https://github.com/evoelsewhere/evo-conductor/commit/6149a2c39b60a5b5feaecbdece1d707079e3a4be) |
| Idempotent scoped `POST /api/v1/telemetry/batch`, owner/installation checks and server receipt timestamp | `telemetry_is_idempotent_private_and_queryable_by_member` and `telemetry_rejects_sensitive_or_cross_owner_payloads` |
| Indexed persistence by member/time, request and installation/time | Conductor commit [`ea39b6b`](https://github.com/evoelsewhere/evo-conductor/commit/ea39b6bbb4188049839f740958956c32d08ea42e) |
| Durable bounded EvoFlux outbox with atomic file replacement, retry and idempotent batch export | EvoFlux commits [`fb1b0be7`](https://github.com/evoelsewhere/evoflux/commit/fb1b0be70889156ee4431a4c7cb0b4e9f744595b) and [`919d8ede`](https://github.com/evoelsewhere/evoflux/commit/919d8ede498da7f0dc29b0e28f94daed49a3ab27) |

| Remaining gap | Affected criteria |
|---|---|
| Aggregation currently filters/groups on client `reported_at`, not server `received_at` | AC-3 |
| Queue drops oldest items at its bound but does not report the dropped count | AC-5 |
| Stable project/resource/version attribution, Plugin installation/tool ownership, explicit request/session boundaries, member-role snapshot, estimated cost and EvoFlux version are absent from the event contract | AC-6, AC-11–AC-16 |
| No replay-burst/load test has established the required throughput | AC-8 |
| The flush path defers every HTTP rejection, so permanent malformed/4xx batches are not terminally classified | AC-10 |

### Acceptance progress

| AC | State |
|---|---|
| AC-1, AC-2, AC-4, AC-7, AC-9 | Implemented in review |
| AC-3, AC-5, AC-6, AC-10 | Partial |
| AC-8 | Not verified |
| AC-11–AC-16 | Not implemented |

## 4. Acceptance criteria

| ID | Criterion |
|---|---|
| AC-1 | `POST /api/v1/telemetry/batch` accepts a batch of events authenticated by a token carrying the `report_telemetry` scope |
| AC-2 | Each event carries a client-generated event identifier; resubmitting the same identifier does not create a second row |
| AC-3 | Each event records both `client_reported_at` and a server-assigned `server_received_at`, and aggregation keys on server time |
| AC-4 | EvoFlux buffers events locally while offline and replays them on reconnection with no loss and no duplication |
| AC-5 | The local buffer has a bounded size; when exceeded, the oldest events are dropped and the number dropped is reported |
| AC-6 | Events carry authenticated project/member identity, installation, session/request/run correlation, event type and sequence, model provider and requested/response model, input/output/cache/reasoning/tool-use token counts, optional client-reported estimated-cost components, tool name and category, Agent/Skill/Plugin resource/version/relation identity, Plugin installation and contributed tool/Skill identity, status, duration, sanitized error category, session start/end and EvoFlux version |
| AC-7 | Indexes exist for the query patterns used by [REQ-016](17-REQ-016-usage-aggregation-dashboards.md), at minimum on installation, user and server timestamp |
| AC-8 | The endpoint sustains a large replay burst when a whole team reconnects after an outage, without failing |
| AC-9 | No field in the payload can carry conversation content, file content or credentials, asserted by a schema test |
| AC-10 | A rejected or malformed batch returns a specific error, and the client does not retry indefinitely on a permanent error |
| AC-11 | A completed user request has exactly one terminal request event; each model and tool call has its own idempotent event linked by request/run/parent identity, so retries remain visible without multiplying the request count |
| AC-12 | Every managed resource attribution includes matching `project_id`, `resource_id` and immutable `version_id` plus kind and relation; Conductor rejects the complete event transaction on a cross-project, inaccessible or unknown resource/version reference |
| AC-13 | The server derives member ID, project ID, primary role, sub-role IDs and tag IDs from authenticated server state and stores an ingestion-time membership snapshot; client-supplied identity or role fields are rejected or ignored and never become query dimensions |
| AC-14 | A Plugin-contributed tool or Skill is attributed through the known managed Plugin installation/resource mapping; an identical tool/Skill name from local or another-project content is not attributed to that Plugin |
| AC-15 | Token and optional client-estimated-cost fields exist only on model-call events, are non-negative and bounded, and preserve input/output/cache/reasoning/tool-use and cost components separately; missing usage or price data is represented as unknown/unpriced, not zero |
| AC-16 | Contract and end-to-end tests execute one request using a managed Agent, standalone Skill and Plugin-contributed tool with multiple model calls including a retry, and prove exact user/project/resource/version attribution, request/model/tool counts, outcomes, token totals, timestamps and no sensitive payload fields |

## 5. Out of scope

- Aggregation and dashboards, covered by [REQ-016](17-REQ-016-usage-aggregation-dashboards.md).
- Cost calculation, covered by [REQ-017](20-REQ-017-cost-estimation.md).
- Retention, covered by [REQ-019](21-REQ-019-data-retention.md).
- Gateway-measured usage, deferred in [REQ-023](23-REQ-023-ai-gateway.md).

## 6. Risks

| # | Risk | Severity | Mitigation |
|---|---|---|---|
| 1 | Replay after an outage double-counts usage | High | AC-2 |
| 2 | Client-reported figures can be inaccurate or disabled by the client | Medium | Document this limitation explicitly; only a gateway can make usage non-repudiable, see [REQ-023](23-REQ-023-ai-gateway.md) |
| 3 | Client clock skew corrupts daily charts | Medium | AC-3 |
| 4 | The raw table grows quickly and queries degrade | Medium | AC-7 plus aggregation and retention |
| 5 | A future field addition quietly introduces content | High | AC-9 as a permanent schema test |
| 6 | One request using several resources or retrying a model is counted several times in project totals | High | Section 2.1 and AC-11 define independent request, resource, model and tool grains |
| 7 | A same-name local or cross-project tool is credited to a governed Plugin | High | AC-12 and AC-14 require stable managed ownership mapping rather than name inference |
| 8 | Role supplied by a modified client falsifies role analysis | High | AC-13 derives role dimensions from server-owned membership state |

## 7. Open questions

- What is the batch trigger: elapsed time, event count, or both? A batch at most every sixty seconds or
  when a threshold count is reached, whichever comes first, is proposed.
- Should token counts be reported per turn or aggregated per session before upload? Per turn gives better
  analysis; per session is cheaper. Per turn is recommended, since the volume is manageable.

## History

| Date | Change | Author |
|---|---|---|
| 2026-08-09 | Created | |
| 2026-08-10 | Recorded partial implementation and remaining gaps from Conductor PR #2 and EvoFlux PR #4 | Codex |
| 2026-08-10 | Accepted by project owner | Project owner |
| 2026-08-11 | Added project-owner-required member/resource attribution, event hierarchy, role, model-call, token, cost and outcome audit contract | Codex |
