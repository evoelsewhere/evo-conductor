# REQ-016 — Usage aggregation and dashboards

| | |
|---|---|
| ID | REQ-016 |
| Created | 2026-08-09 |
| Updated | 2026-08-10 |
| Status | Accepted (2026-08-10) — partial implementation in review |
| Priority | P0 |
| Build order | Step 17 of 23 |
| Spec section | [requirements.md section 11](../requirements.md) |
| Source | Baseline specification section 11 |
| Depends on | REQ-004, REQ-013, REQ-014, REQ-015 |
| Blocks | REQ-017, V1 acceptance criterion 10 |
| Repositories | `evo-conductor` |
| Design | Not created; requirement accepted 2026-08-10, so an as-built reconciliation design is the next lifecycle artifact |

## 1. Context

Once telemetry arrives it must be readable. Querying the raw event table directly will degrade within
weeks at one event per turn across a team, so the aggregate layer is part of the initial design rather
than a later optimization. Building it afterwards means rewriting every query first written against the
raw table.

## 2. Requirement

Conductor shall aggregate telemetry into a queryable daily model and shall present project overview and
usage dashboards with the filters and metrics defined in specification section 11. A User shall see only
their own usage unless explicitly authorized.

## 3. Implementation status

The project owner accepted this requirement on 2026-08-10. A per-member audit slice is open in
[evo-conductor#2](https://github.com/evoelsewhere/evo-conductor/pull/2). It is useful product value, but it
does not yet implement the aggregate architecture or project-wide scope required by this requirement.

| Delivered in review | Evidence |
|---|---|
| Member overview KPIs for requests, token categories, model calls, tool calls and errors | Conductor commits [`352e5c6`](https://github.com/evoelsewhere/evo-conductor/commit/352e5c6e6308358a2a031bf5110a988d03549d98) and [`e3c36e5`](https://github.com/evoelsewhere/evo-conductor/commit/e3c36e51b3ca1a4882c688690ac3bf86b78f478d) |
| Daily token trend and model/provider distribution charts | Reusable Recharts components in Conductor PR #2 |
| Request activity, pagination, request detail and per-event audit timeline | Member activity and request-detail routes/pages in Conductor PR #2 |
| Tool totals, success/failure, average duration and ranked usage chart | Member tools API and page in Conductor PR #2 |
| Preset and custom date-range filters, responsive mobile layout and local provider icons | [Playwright QA evidence](https://github.com/evoelsewhere/evo-conductor/blob/c5431d3bc2070ff704f29fd374e6f28da0dc2781/docs/member-usage-ui-qa.md) |
| Member-scoped authorization and admin-managed member connection tokens | Telemetry and member-secret route tests in Conductor PR #2 |

| Remaining gap | Affected criteria |
|---|---|
| No `usage_aggregates` table or automatic late-arrival recomputation; current queries scan `telemetry_events` and group on client time | AC-1, AC-2, AC-3 |
| No complete project overview, project/team/tag/sub-role/agent/installation filters or highest-usage resource views | AC-4, AC-5, AC-6 |
| Resource adoption and MCP server/tool analytics are not implemented | AC-8, AC-12 |
| Empty-state distinction is only partial and has no dedicated automated assertion | AC-9 |
| No projected-volume target or benchmark has been recorded | AC-11 |

### Acceptance progress

| AC | State | Note |
|---|---|---|
| AC-7, AC-10 | Implemented in review | Self-only access is allowed by the privacy boundary; privileged roles may inspect another member |
| AC-4, AC-5, AC-6, AC-9, AC-12 | Partial | Per-member slice only |
| AC-1, AC-2, AC-3, AC-8, AC-11 | Not implemented or not verified | Aggregate/project scope remains |

## 4. Acceptance criteria

| ID | Criterion |
|---|---|
| AC-1 | A `usage_aggregates` table summarizes by project, user, installation, date, provider and model |
| AC-2 | Aggregation runs automatically, and events arriving late after an offline period are attributed to their own date rather than the date of arrival |
| AC-3 | Every chart reads from the aggregate table; no dashboard query scans the raw event table |
| AC-4 | The project overview shows member counts by status, connected installations, active tokens, resource counts by type, synchronization health, SSO status and recent administrative activity |
| AC-5 | The usage dashboard filters by date range, member, team or tag or sub-role, model and provider, agent, tool, MCP server, and installation |
| AC-6 | Metrics include input and output tokens, sessions, tool calls, tool success and failure rate, average tool duration, active agents, trend over time, and highest-usage members and resources |
| AC-7 | A User sees only personal usage; project-wide endpoints return `403` per [REQ-004](02-REQ-004-api-authorization.md) |
| AC-8 | Resource adoption is visible: for each resource, how many installations hold it and at which version, and which have not synchronized it |
| AC-9 | Empty states distinguish "no installation has connected yet" from "no activity in the selected range" |
| AC-10 | Default range is the last thirty days |
| AC-11 | Dashboards remain responsive at the projected data volume, with a stated target and a measurement |
| AC-12 | A per-member view shows, in one place, that member's connection tokens, tool usage and MCP server and tool usage; access is restricted to Admin per [REQ-004](02-REQ-004-api-authorization.md) |

## 5. Out of scope

- Cost, covered by [REQ-017](20-REQ-017-cost-estimation.md).
- Individual productivity scoring or ranking of members. This is deliberately excluded; see
  [REQ-015](11-REQ-015-privacy-controls.md) risk 1.
- Real-time alerting and anomaly detection. Reconsider at P2.
- Export to an external BI system. Reconsider at P2.

## 6. Risks

| # | Risk | Severity | Mitigation |
|---|---|---|---|
| 1 | Late-arriving events corrupt totals | Medium | AC-2 recomputes per date rather than incrementing blindly |
| 2 | A dashboard that shows zero forever destroys confidence, as the current one does | Medium | AC-9 |
| 3 | Adding breakdown dimensions explodes aggregate cardinality | Low | AC-1 fixes the stored dimensions; other dimensions are computed at query time |
| 4 | The dashboard is read as a performance ranking of individuals | High | Section 5 excludes it explicitly, and [REQ-015](11-REQ-015-privacy-controls.md) AC-7 makes the data symmetric |

## 7. Open questions

- Is aggregation computed incrementally at ingestion, or by a scheduled job? Incremental accumulation
  plus a nightly recomputation to absorb late arrivals is recommended.
- **Partly settled.** Acceptance criterion 11 confirms Admin drill-down into a member's tokens, tool usage
  and MCP usage, which AC-12 now covers. Whether a Contributor sees per-member figures or only project
  totals is still open; this mirrors [REQ-004](02-REQ-004-api-authorization.md) and must be answered once for
  both.

## History

| Date | Change | Author |
|---|---|---|
| 2026-08-09 | Created | |
| 2026-08-10 | Recorded the implemented per-member audit slice and remaining aggregate/project gaps | Codex |
| 2026-08-10 | Accepted by project owner | Project owner |
