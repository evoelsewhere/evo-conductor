# REQ-004 — API-enforced authorization

| | |
|---|---|
| ID | REQ-004 |
| Created | 2026-08-09 |
| Updated | 2026-08-09 |
| Status | Draft |
| Priority | P0 |
| Build order | Step 2 of 23 |
| Spec section | [requirements.md sections 3 and 7](../requirements.md) |
| Source | Baseline specification sections 3 and 7, plus code review 2026-08-09 |
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

| Implemented | Missing | Incorrect |
|---|---|---|
| Role guards on members, tags, sub-roles, settings and SSO ([users.rs](../../crates/conductor-server/src/http/routes/users.rs), [access.rs](../../crates/conductor-server/src/http/routes/access.rs), [settings.rs](../../crates/conductor-server/src/http/routes/settings.rs)) | A guard on `GET /api/dashboard` | `GET /api/dashboard` requires only an authenticated session ([dashboard.rs:8-13](../../crates/conductor-server/src/http/routes/dashboard.rs)) |
| Capability predicates in the domain layer ([role.rs:33-56](../../crates/conductor-domain/src/role.rs)) | A guard on `GET /api/resources` | `can_view_telemetry()` is defined but is never called anywhere in the codebase |
| Member list already narrows results for non-managers ([users.rs:45-54](../../crates/conductor-server/src/http/routes/users.rs)) | Role constraints on token issuance | `POST /api/secrets` performs no role check and grants all three scopes when `scopes` is omitted ([secrets.rs:31-38](../../crates/conductor-server/src/http/routes/secrets.rs)) |

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
  that Admin views an individual member's tokens, tool usage and MCP usage, so Admin drill-down is
  confirmed. What remains open is whether a Contributor also gets per-member drill-down or only
  project-level totals. Recommendation: project totals only for Contributor.

## History

| Date | Change | Author |
|---|---|---|
| 2026-08-09 | Created | |
