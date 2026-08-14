# REQ-016 — Usage aggregation and dashboards

| | |
|---|---|
| ID | REQ-016 |
| Created | 2026-08-09 |
| Updated | 2026-08-14 |
| Status | Accepted — portfolio Analytics Studio and drill-downs implemented; aggregate layer remains open |
| Priority | P0 |
| Build order | Step 17 of 23 |
| Spec section | [requirements.md section 11](../requirements.md) |
| Source | Baseline specification section 11; project-owner member/resource audit and chart extension 2026-08-11 |
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
their own usage unless explicitly authorized. Authorized viewers shall be able to move consistently from
project totals, to a member or resource breakdown, to one privacy-safe request timeline and determine who
used each Agent, Skill or Plugin version, when it happened, how much it used, which model/tool calls ran,
and whether they succeeded or failed.

### 2.1 Metrics and counting semantics

Dashboard labels and API fields shall use these definitions consistently:

| Metric | Definition |
|---|---|
| Requests | Distinct terminal `request_id` values in the selected range |
| Resource uses | Distinct `(request_id, resource_id, version_id, relation)` attributions; may overlap across resources |
| Model calls | Model-call events, including retries; displayed separately from Requests |
| Tool calls | Tool-call events; success/error/blocked use tool outcome, not request outcome |
| Success rate | Successful terminal Requests divided by terminal Requests; cancelled is shown separately |
| Error rate | Failed terminal Requests divided by terminal Requests; sanitized error category is a breakdown |
| Total tokens | Sum of model-call input, output, cache-read, reasoning and tool-use token fields without re-adding request totals |
| Estimated cost | Sum of priced model-call estimates only; unpriced calls and the estimate source remain visible |
| Average per request | The applicable total divided by distinct Requests with a nonzero denominator |
| Duration | Request duration for request KPIs and tool/model duration for their own views; averages never mix grains |

The dashboard shall expose total and per-request averages for model calls, tokens and estimated cost, as
well as average request duration. Resource-attributed totals shall display an overlap notice and shall
never be added together to claim a project total.

### 2.2 Filters and analysis views

The project usage view, member detail, resource detail and request activity views shall share a canonical
filter model encoded in the URL so a filtered analysis can be linked and restored. Filters include:

- time preset (`today`, current week, current month, last 7/30/90 days) or custom `from`/`to`, applied to
  authoritative `server_received_at` with the viewer timezone displayed; activity detail also shows the
  client-reported occurrence time;
- member, primary role recorded at ingestion, sub-role and member tag;
- resource kind, resource, immutable version and release channel, with separate Agent, Skill and Plugin
  selectors;
- provider, requested/response model, event type, request/tool status and sanitized error category;
- tool/contributed tool, EvoFlux installation and client version.

Filters compose with AND across dimensions and OR within a multi-select dimension. Every KPI, chart,
ranking and table on the view uses the same effective filter and date boundary. Reset, empty, loading,
forbidden and error states are explicit. Presets and custom ranges support comparison with the immediately
preceding equal-length period.

### 2.3 Charts and drill-down

The main usage view and relevant member/resource detail views shall provide reusable, accessible charts:

1. request trend as stacked success/error/cancelled series;
2. stacked token trend for input, output, cache-read, reasoning and tool-use tokens;
3. estimated-cost trend plus total estimated cost and unpriced model-call count;
4. provider/model distribution by model calls, tokens and estimated cost;
5. Agent/Skill/Plugin attributed-usage share as a donut and ranked horizontal bar, with the overlap notice;
6. success/error rate and average/p95 duration by resource and model;
7. top resources and members, with role breakdown available without turning the view into a productivity score.

Charts use the existing frontend chart library and shared components, not hand-written SVG. Tooltips,
legends, keyboard access, tabular fallback, provider icons and non-colour status labels are required.
Clicking a chart segment applies the corresponding filter and updates the activity table.

The activity table displays timestamp, member, recorded role, resource kind/name/version and attribution
relation, request outcome, model-call count, model/provider, total token categories, estimated cost with
source, duration and sanitized error category. It supports server-side pagination and deterministic sort.
The request detail shows the correlated request, Agent runs, activated standalone or Plugin-provided
Skills, Plugin-contributed tools, model-call/retry timeline, per-call tokens/cost/duration and tool
outcomes. It never shows prompts, responses, reasoning text, tool arguments/results or file paths.

## 3. Implementation status

The current console includes both the original member audit slice and the newer project/resource
portfolio analytics.

| Implemented | Evidence |
|---|---|
| Member overview, daily token/model charts, tools, paginated activity and privacy-safe request detail | Current member usage/activity/tool pages and [member-usage-ui-qa.md](../member-usage-ui-qa.md) |
| Project `GET /api/analytics/resource-usage` with server-time range and member, recorded role, resource kind/ID/version, status, provider/model, installation, relation and tool filters | `ResourceUsageRepo` plus `resource_usage_analytics_attributes_member_role_version_tokens_and_cost` |
| Portfolio totals, trends, resource/member/model/role/tool breakdowns, activity and pagination with separate request/resource/model/tool grains, token categories, cost and unpriced calls | Resource analytics domain/repository and Analytics Studio panels |
| Resource detail combines adoption/inventory, usage, version history, member breakdown and feedback | Resource governance/monitoring routes and UI evidence |
| Analytics Studio composes allowlisted KPI, line, area, bar, stacked-bar, donut and table widgets | `resource-analytics-studio.tsx` and typed analytics definitions |
| Private/shared saved views are project-scoped and owner-attributed with role enforcement and optimistic revision conflicts | `analytics_views` route/repository tests |
| A 1,000-member reference workload measured portfolio and Skill-scoped analytics after 9,000 accepted telemetry events | [fleet-simulator.md](../fleet-simulator.md) |

| Remaining gap | Affected criteria |
|---|---|
| No `usage_aggregates` table/job; portfolio queries still scan raw events and personal queries still use client time | AC-1–AC-3 |
| The generic dashboard route still exposes project summary to a plain User; project usage itself is correctly guarded | AC-4, AC-7 |
| URL persistence, comparison deltas, timezone/delay presentation, click-to-filter and accessible table equivalents are incomplete across all panels | AC-5, AC-14–AC-18, AC-22, AC-25 |
| Sub-role/tag/EvoFlux-version filtering and complete correlated Agent/Skill/Plugin per-call request detail remain incomplete | AC-5, AC-18–AC-21 |
| Saved dashboard layout is typed and revisioned, but there is no frontend unit/e2e framework in this repository | AC-25, REQ-020 |
| Reference volume is measured, but no accepted performance SLO or CI benchmark exists | AC-11 |

### Acceptance progress

| AC | State | Note |
|---|---|---|
| AC-6–AC-10, AC-12, AC-13, AC-16, AC-19, AC-21, AC-23, AC-24 | Implemented or substantially implemented | Portfolio/member/resource scopes now exist |
| AC-4, AC-5, AC-11, AC-14, AC-15, AC-17, AC-18, AC-20, AC-22, AC-25 | Partial | See residual UI/filter/detail/test gaps above |
| AC-1–AC-3 | Not implemented | Normalized aggregate storage/job is still absent |

## 4. Acceptance criteria

| ID | Criterion |
|---|---|
| AC-1 | A normalized `usage_aggregates` family summarizes by project, user, ingestion-time primary role/sub-role/tag snapshot, installation, date, provider/model, request outcome and managed Agent/Skill/Plugin resource/version/relation without combining incompatible request/model/tool grains |
| AC-2 | Aggregation runs automatically and keys on authoritative server receipt date; late offline events are included idempotently on receipt while retaining their client occurrence timestamp for activity detail and delay analysis |
| AC-3 | Every chart reads from the aggregate table; no dashboard query scans the raw event table |
| AC-4 | The project overview shows member counts by status, connected installations, active tokens, resource counts by type, synchronization health, SSO status and recent administrative activity |
| AC-5 | The usage dashboard implements the canonical URL-backed filter model in section 2.2, including preset/custom date range, member and role dimensions, Agent/Skill/Plugin resource and version, provider/model, event/outcome/error, tool, installation and EvoFlux version |
| AC-6 | Metrics include distinct requests and resource uses, model/tool calls, request and tool success/error/cancelled rates, input/output/cache/reasoning/tool-use tokens, estimated cost and unpriced calls, total and per-request averages, average/p95 duration, active agents, trend and highest-usage members/resources |
| AC-7 | A User sees only personal usage; project-wide endpoints return `403` per [REQ-004](02-REQ-004-api-authorization.md) |
| AC-8 | Resource adoption is visible: for each resource, how many installations hold it and at which version, and which have not synchronized it |
| AC-9 | Empty states distinguish "no installation has connected yet" from "no activity in the selected range" |
| AC-10 | Default range is the last thirty days |
| AC-11 | Dashboards remain responsive at the projected data volume, with a stated target and a measurement |
| AC-12 | A per-member view shows, in one place, that member's connection tokens, tool usage and Plugin/contributed-tool usage; access is restricted to Admin per [REQ-004](02-REQ-004-api-authorization.md) |
| AC-13 | Project, member and resource usage endpoints return the same metric definitions from section 2.1; automated fixtures containing retries and several resource attributions prove project totals are not double-counted |
| AC-14 | Date presets include today, current week, current month and last 7/30/90 days; custom `from`/`to` is inclusive/exclusive as documented, viewer timezone is visible, and the activity row shows both client occurrence and server receipt when they materially differ |
| AC-15 | Filters compose as specified, are encoded in the URL, drive every KPI/chart/table consistently, can be reset, and reject invalid/cross-project IDs server-side rather than silently ignoring them |
| AC-16 | KPI cards show Requests, Resource uses, Model calls, Total tokens, Estimated cost, Success rate, Errors and Average duration, with previous-period deltas and exact metric/tooltips explaining denominators |
| AC-17 | The seven chart analyses in section 2.3 render from aggregate endpoints, support click-to-filter, expose accessible table equivalents and remain legible on mobile without hiding totals or filter state |
| AC-18 | Member and resource rankings can be grouped/filtered by recorded primary role, sub-role and tag; role labels state that they are the server-owned membership snapshot at ingestion, and individual productivity scoring is never calculated |
| AC-19 | The activity endpoint is server-paginated and returns timestamp, member/recorded role, resource/version/relation, request outcome, model-call count, provider/model, separated token counts, cost/source, duration and sanitized error category for each row |
| AC-20 | Request detail correlates request, run, model and tool events in deterministic sequence and shows managed Agent, standalone Skill, Plugin, Plugin-contributed Skill/tool attribution and per-call outcome/tokens/cost/duration without exposing work content |
| AC-21 | A member detail integrates that member's installations, tokens, resource usage KPIs/charts and activity; a resource detail shows members, roles, versions, adoption, usage and failures for that Agent, Skill or Plugin |
| AC-22 | Clicking a member, resource, model, outcome or chart segment navigates or filters without losing the selected date range; browser back/forward restores the previous analysis state |
| AC-23 | Estimated cost displays the source (`client_reported`, `conductor_priced` or `unpriced`), currency and pricing effective date where known; unpriced model calls are shown separately and are never silently counted as zero |
| AC-24 | A regular User can access the same personal activity fields and charts for themselves that Admin can see about them, while unauthorized cross-member/project requests return `403` and are audit logged |
| AC-25 | Playwright coverage captures desktop and mobile screenshots for populated, filtered, empty, loading, forbidden and error states and verifies chart click-through to a matching request detail row |

## 5. Out of scope

- Price-table management, repricing and budget alerts, covered by
  [REQ-017](20-REQ-017-cost-estimation.md). This requirement still displays allowlisted client-reported
  estimates and later prefers Conductor-priced estimates with an explicit source label.
- Individual productivity scoring or ranking of members. This is deliberately excluded; see
  [REQ-015](11-REQ-015-privacy-controls.md) risk 1.
- Real-time alerting and anomaly detection. Reconsider at P2.
- Export to an external BI system. Reconsider at P2.

## 6. Risks

| # | Risk | Severity | Mitigation |
|---|---|---|---|
| 1 | Late-arriving events corrupt totals or appear under an untrusted client date | Medium | AC-2 uses server receipt date for aggregates and retains client occurrence time separately |
| 2 | A dashboard that shows zero forever destroys confidence, as the current one does | Medium | AC-9 |
| 3 | Adding breakdown dimensions explodes aggregate cardinality | Low | AC-1 fixes the stored dimensions; other dimensions are computed at query time |
| 4 | The dashboard is read as a performance ranking of individuals | High | Section 5 excludes it explicitly, and [REQ-015](11-REQ-015-privacy-controls.md) AC-7 makes the data symmetric |
| 5 | Resource charts double-count project totals because one request uses an Agent, Skill and Plugin | High | Section 2.1 and AC-13 separate project facts from overlapping attribution |
| 6 | A current role is displayed beside old usage and misrepresents historical context | Medium | AC-18 labels and filters the server-owned ingestion-time role snapshot |
| 7 | Cost appears precise even though the model is unpriced or the client estimate is stale | High | AC-23 displays source/effective date and keeps unpriced calls visible |
| 8 | Rich request detail becomes a route for prompt, tool payload or path collection | High | AC-20 and [REQ-015](11-REQ-015-privacy-controls.md) enforce the typed metadata-only boundary |

## 7. Open questions

- Is aggregation computed incrementally at ingestion, or by a scheduled job? Incremental accumulation
  plus a nightly recomputation to absorb late arrivals is recommended.
- **Partly settled.** Acceptance criterion 11 confirms Admin drill-down into a member's tokens, tool usage
  and Plugin usage, which AC-12 now covers. Whether a Contributor sees per-member figures or only project
  totals is still open; this mirrors [REQ-004](02-REQ-004-api-authorization.md) and must be answered once for
  both.

## History

| Date | Change | Author |
|---|---|---|
| 2026-08-09 | Created | |
| 2026-08-10 | Recorded the implemented per-member audit slice and remaining aggregate/project gaps | Codex |
| 2026-08-10 | Accepted by project owner | Project owner |
| 2026-08-11 | Added project-owner-required member/resource usage audit, metric grain, role filters, charts, cost source and request drill-down | Codex |
| 2026-08-14 | Reconciled Analytics Studio, saved views, portfolio/resource analytics and the measured fleet run while retaining aggregate/UI gaps | Codex |
