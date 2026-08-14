# REQ-004 — API-enforced authorization

| | |
|---|---|
| ID | REQ-004 |
| Created | 2026-08-09 |
| Updated | 2026-08-14 |
| Status | Draft — partial implementation in source |
| Priority | P0 |
| Build order | Step 2 of 23 |
| Spec section | [requirements.md sections 3 and 7](../requirements.md) |
| Source | Baseline specification sections 3 and 7, code review 2026-08-09, project-owner usage-audit extension 2026-08-11 |
| Depends on | none |
| Blocks | REQ-008, REQ-016, V1 acceptance criteria 6 and 13 |
| Repositories | `evo-conductor` |
| Design | Not created; requires acceptance |

## 1. Context

The specification states that a User shall not view project-wide telemetry unless explicitly authorized,
and that access checks must be enforced by the Rust API because frontend route hiding is not sufficient
security. The current code satisfies neither statement for the endpoints that matter most.

The capability predicates already exist in the domain layer. The gap is that the most valuable endpoints
never call them.

## 2. Requirement

Every endpoint that exposes project-wide data or performs a privileged action shall verify the caller's
role on the server. Connection-token issuance shall be constrained by role, and scopes shall be declared
explicitly rather than defaulted.

## 3. Implementation status

| Implemented | Missing or incorrect |
|---|---|
| Project analytics, saved analytics views and resource inventory endpoints call `can_view_telemetry()`; member usage routes allow self or a telemetry-capable role | `GET /api/dashboard` still requires only an authenticated session, so AC-1 remains open |
| `GET /api/resources` now uses `list_for_actor`, while client delivery uses the server-side effective-audience resolver | There is no generated, exhaustive three-role matrix for every route |
| Resource authoring is Admin/Contributor only; a Contributor may mutate only owned resources; settings and member management retain their role guards | Connection-secret scopes are explicit and non-empty, but role-to-scope constraints are not defined or enforced |
| Secret creation rejects an absent/empty scope list; member-secret management is self-or-Admin | Permission denials are not persisted because REQ-018 remains incomplete |
| The console uses role-aware navigation/action visibility for implemented management surfaces | Several guards remain handler-local rather than enforced by a common route policy layer |

### Acceptance progress

| AC | State |
|---|---|
| AC-2, AC-3, AC-4, AC-7, AC-9 | Implemented for the current usage/resource surfaces |
| AC-6 | Partial — focused negative tests exist, not a complete route matrix |
| AC-1, AC-5, AC-8 | Not complete |

## 4. Acceptance criteria

| ID | Criterion |
|---|---|
| AC-1 | A User calling `GET /api/dashboard` receives `403`; Contributor and Admin receive `200` |
| AC-2 | A User calling a project-wide usage endpoint receives `403`; a personal usage endpoint returns only their own data |
| AC-3 | `GET /api/resources` returns only resources the caller is permitted to see, per [REQ-008](10-REQ-008-resource-access-policy.md) |
| AC-4 | Token creation requires an explicit non-empty `scopes` array; an empty or absent array returns `400` |
| AC-5 | A User may only request scopes permitted for their role; requesting a broader scope returns `403` |
| AC-6 | An authorization regression test exists for every endpoint against all three primary roles |
| AC-7 | The console hides navigation entries and actions the current role cannot use, as a usability measure layered on top of server enforcement and never as a substitute for it |
| AC-8 | Rejections caused by insufficient permission are recorded in the audit log ([REQ-018](05-REQ-018-audit-logging.md)) |
| AC-9 | Admin can access another member's privacy-safe resource/request usage drill-down defined by [REQ-016](17-REQ-016-usage-aggregation-dashboards.md); a regular User receives `403` for another member and can retrieve the same fields only for themselves |

## 5. Out of scope

- Sub-role-based permissions on endpoints. Sub-roles are used for resource targeting in
  [REQ-008](10-REQ-008-resource-access-policy.md), not for endpoint authorization.
- Per-resource ownership rules beyond the visibility model.

## 6. Risks

| # | Risk | Severity | Mitigation |
|---|---|---|---|
| 1 | Tightening breaks a flow currently relied upon | Low | AC-6 establishes the full matrix before behaviour changes |
| 2 | Guards remain scattered per handler and a new endpoint is added without one | Medium | The design should evaluate a `RequireRole` extractor instead of repeated inline checks |
| 3 | Frontend hiding is mistaken for enforcement | High | AC-7 states the relationship explicitly; AC-6 tests the server directly |

## 7. Open questions

- **Partly settled.** Acceptance criterion 11 in [requirements.md section 16](../requirements.md) states
  that Admin audits an individual member's Agent/Skill/Plugin use, outcomes, model calls, tokens,
  estimated cost and recorded role, so Admin drill-down is confirmed. What remains open is whether a
  Contributor also gets per-member drill-down or only project-level totals. Recommendation: project
  totals only for Contributor.

## History

| Date | Change | Author |
|---|---|---|
| 2026-08-09 | Created | |
| 2026-08-11 | Required Admin member/resource usage drill-down while preserving self-only access for regular Users | Codex |
| 2026-08-14 | Reconciled resource/analytics/member guards and retained the dashboard, scope-policy and audit gaps | Codex |
