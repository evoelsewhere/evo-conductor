# REQ-014 — Telemetry ingestion

| | |
|---|---|
| ID | REQ-014 |
| Created | 2026-08-09 |
| Updated | 2026-08-09 |
| Status | Draft |
| Priority | P0 |
| Spec section | [requirements.md section 9](../requirements.md) |
| Source | Baseline specification section 9 |
| Depends on | REQ-001, REQ-011, REQ-015 |
| Blocks | REQ-016, REQ-017, V1 acceptance criterion 7 |
| Repositories | `evo-conductor` and `evoflux` |
| Design | Not created; requires acceptance |

## 1. Context

Monitoring is the capability the project owner has identified as most important, and it is currently the
emptiest part of the system. A table and a type exist; there is no ingestion endpoint, no index, no
aggregation and no client.

Two properties of the environment shape the design and are easy to overlook. EvoFlux is a local-first
desktop application that will regularly be offline, and its clock cannot be trusted.

## 2. Requirement

EvoFlux shall send batched usage telemetry to Conductor. Ingestion shall be idempotent, shall tolerate
replay after network interruption, and shall record both client-reported and server-assigned timestamps.

## 3. Implementation status

| Implemented | Missing | Incorrect |
|---|---|---|
| `telemetry_events` table with `tokens_in`, `tokens_out`, `tool_calls`, `active_agents`, `reported_at` ([migrate.rs:133-143](../../crates/conductor-storage/src/migrate.rs)) | Every field required by specification section 9: tool name and category, MCP server and tool, status and duration, model and provider, installation, session times, error category, EvoFlux version | The table has no index of any kind, while five indexes were created for `users` and `tags` in the same migration |
| `TelemetrySnapshot` type carrying counters only, correctly content-free ([telemetry.rs](../../crates/conductor-domain/src/telemetry.rs)) | `POST /api/v1/telemetry/batch` | |
| `report_telemetry` scope defined | Idempotency, offline buffering, server timestamps | |

## 4. Acceptance criteria

| ID | Criterion |
|---|---|
| AC-1 | `POST /api/v1/telemetry/batch` accepts a batch of events authenticated by a token carrying the `report_telemetry` scope |
| AC-2 | Each event carries a client-generated event identifier; resubmitting the same identifier does not create a second row |
| AC-3 | Each event records both `client_reported_at` and a server-assigned `server_received_at`, and aggregation keys on server time |
| AC-4 | EvoFlux buffers events locally while offline and replays them on reconnection with no loss and no duplication |
| AC-5 | The local buffer has a bounded size; when exceeded, the oldest events are dropped and the number dropped is reported |
| AC-6 | Events carry user, installation, session, model provider and model, token counts, tool name and category, MCP server and tool name, tool status and duration, active agents, session start and end, error category and EvoFlux version |
| AC-7 | Indexes exist for the query patterns used by [REQ-016](REQ-016-usage-aggregation-dashboards.md), at minimum on installation, user and server timestamp |
| AC-8 | The endpoint sustains a large replay burst when a whole team reconnects after an outage, without failing |
| AC-9 | No field in the payload can carry conversation content, file content or credentials, asserted by a schema test |
| AC-10 | A rejected or malformed batch returns a specific error, and the client does not retry indefinitely on a permanent error |

## 5. Out of scope

- Aggregation and dashboards, covered by [REQ-016](REQ-016-usage-aggregation-dashboards.md).
- Cost calculation, covered by [REQ-017](REQ-017-cost-estimation.md).
- Retention, covered by [REQ-019](REQ-019-data-retention.md).
- Gateway-measured usage, deferred in [REQ-023](REQ-023-ai-gateway.md).

## 6. Risks

| # | Risk | Severity | Mitigation |
|---|---|---|---|
| 1 | Replay after an outage double-counts usage | High | AC-2 |
| 2 | Client-reported figures can be inaccurate or disabled by the client | Medium | Document this limitation explicitly; only a gateway can make usage non-repudiable, see [REQ-023](REQ-023-ai-gateway.md) |
| 3 | Client clock skew corrupts daily charts | Medium | AC-3 |
| 4 | The raw table grows quickly and queries degrade | Medium | AC-7 plus aggregation and retention |
| 5 | A future field addition quietly introduces content | High | AC-9 as a permanent schema test |

## 7. Open questions

- What is the batch trigger: elapsed time, event count, or both? A batch at most every sixty seconds or
  when a threshold count is reached, whichever comes first, is proposed.
- Should token counts be reported per turn or aggregated per session before upload? Per turn gives better
  analysis; per session is cheaper. Per turn is recommended, since the volume is manageable.

## History

| Date | Change | Author |
|---|---|---|
| 2026-08-09 | Created | |
