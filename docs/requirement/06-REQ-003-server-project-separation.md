# REQ-003 — Server and project configuration separation

| | |
|---|---|
| ID | REQ-003 |
| Created | 2026-08-09 |
| Updated | 2026-08-14 |
| Status | Draft — resource/data-plane project scoping partially implemented |
| Priority | P0 |
| Build order | Step 6 of 23 |
| Spec section | [requirements.md section 1](../requirements.md) |
| Source | Baseline specification section 1, plus code review 2026-08-09 |
| Depends on | REQ-001 |
| Blocks | none, but the cost of deferring it rises with every stored row |
| Repositories | `evo-conductor` |
| Design | Not created; requires acceptance |

## 1. Context

The specification fixes V1 at one deployment per project workspace and states that multi-project support
can be introduced later. The current code satisfies V1, but it does so in a way that makes "later" very
expensive: the constraint is expressed through the authentication layer and through global uniqueness
rather than through a policy decision.

Migration cost is asymmetric. Moving from multi-project to single-project is trivial. Moving from
single-project to multi-project touches identity, the token signing key and every slug namespace. The
system currently holds no production data and has issued no tokens, which is the cheapest moment this
decision will ever have.

## 2. Requirement

Conductor shall separate server operating configuration from project business configuration, shall keep
user identity global so that one person can later belong to several projects with one account, and shall
carry a project scope on business data. V1 shall continue to expose a single project and shall not
present multi-project navigation.

## 3. Implementation status

| Implemented | Missing | Incorrect |
|---|---|---|
| Stable project identity remains `instance.id` for the one-project-per-deployment V1 model | Separate `server_config`, `projects` and `project_members` tables | `bind_host`, `bind_port` and `jwt_secret` still share the singleton `instance` row |
| Resources, immutable versions, release channels, Beta targets, policies, changes, installation inventory, telemetry attribution and saved analytics views carry project scope | Global identity versus project membership separation | `users`, `sub_roles`, `tags` and SSO configuration remain deployment-global singletons |
| Resource slug uniqueness is now `UNIQUE(project_id, kind, slug)` and delivery derives project from the authenticated installation/token | Database-enforced composite project foreign keys across every table | Several singleton updates intentionally have no `WHERE` clause and are safe only while one deployment serves one project |
| Cross-project resource/inventory/telemetry identifiers are rejected at the API/repository boundary | Multi-project login/session routing and UI | The schema is prepared for project-scoped delivery, not a complete multi-project server |

## 4. Acceptance criteria

| ID | Criterion |
|---|---|
| AC-1 | `jwt_secret`, `bind_host` and `bind_port` are moved to a `server_config` table |
| AC-2 | A `projects` table exists; the current `instance` row is migrated into it as the first project with no data loss |
| AC-3 | A `project_members(user_id, project_id, primary_role)` table exists; user identity remains global, one account per person |
| AC-4 | `resources`, `tags`, `sub_roles`, `telemetry_events` and installation tables carry `project_id` |
| AC-5 | Uniqueness constraints become project-scoped, for example `UNIQUE(project_id, kind, slug)` |
| AC-6 | No write query executes without a determinate `WHERE` clause |
| AC-7 | Observable behaviour is unchanged: one project, identical console, identical API responses |
| AC-8 | Authentication regression tests for password sign-in and SSO sign-in pass unchanged |
| AC-9 | Every resource, version, release-channel binding, access assignment, client installation, synchronization state and inventory row has a database-enforced project association; a cross-project foreign-key or lookup attempt is rejected rather than falling back to a global slug |

## 5. Out of scope

- Multi-project navigation or a project switcher in the console.
- Inviting one person into several projects.
- Per-project SSO configuration.

These wait until a second project actually exists. This requirement only ensures the schema does not
block that.

## 6. Risks

| # | Risk | Severity | Mitigation |
|---|---|---|---|
| 1 | Touching the authentication layer breaks sign-in | High | AC-8; land [REQ-020](01-REQ-020-automated-testing-ci.md) authentication tests first |
| 2 | Scope expands into building full multi-project support | Medium | Section 5 is binding: schema only, no UI |
| 3 | Deferred, then performed later with live tokens and real data in place | High | This is precisely why the requirement is P0 |
| 4 | A resource or client row is attached to the wrong project and becomes visible or installable across project boundaries | High | AC-4, AC-5 and AC-9 make project scope part of storage identity and authorization |

## 7. Open questions

- Confirm the intended deployment model: one organization running one Conductor across several projects,
  or one deployment per project permanently. If the latter is certain and permanent, AC-2 through AC-5
  can be dropped and only AC-1 and AC-6 retained.

## History

| Date | Change | Author |
|---|---|---|
| 2026-08-09 | Created | |
| 2026-08-14 | Reconciled project-scoped delivery/telemetry rows with the still-singleton server and membership model | Codex |
| 2026-08-11 | Required database-enforced project ownership for resources, client synchronization state and inventory | Codex |
