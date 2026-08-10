# REQ-013 — Inventory synchronization

| | |
|---|---|
| ID | REQ-013 |
| Created | 2026-08-09 |
| Updated | 2026-08-11 |
| Status | Accepted (2026-08-11; owner requested design and task planning) |
| Priority | P0 |
| Build order | Step 14 of 23 |
| Spec section | [requirements.md section 8](../requirements.md) |
| Source | Baseline specification section 8; EvoFlux Portable Agent Plugins update 2026-08-11 |
| Depends on | REQ-001, REQ-011, REQ-012 |
| Blocks | REQ-016, V1 acceptance criteria 7, 8 and 10 |
| Repositories | `evo-conductor` and `evoflux` |
| Design | [DES-007](../design/09-DES-007-governed-resource-delivery.md) sections 4, 6.2 and 10 — Approved 2026-08-11 |

## 1. Context

Inventory answers the operational questions that usage metrics cannot: who is actually connected, who is
running an outdated version, whose synchronization is failing, and where a resource has not yet landed.

The current dashboard already claims to answer the first of these and cannot. It computes
`members_online` from a table that is never written, so the figure is permanently zero. A monitoring
screen displaying a fabricated number is worse than one that displays nothing.

## 2. Requirement

EvoFlux shall periodically report its local project inventory, including observed Conductor-managed
Agent, Skill and Plugin state. Conductor shall accept those reports idempotently,
compare observed state with assigned desired state, and surface installation health to authorized users.
Every reported managed resource shall be attributed to the same authenticated `project_id` used for
synchronization; inventory is observation and can never move a resource or installation between projects.

## 3. Implementation status

| Implemented | Missing | Incorrect |
|---|---|---|
| `member_inventory` table with a small subset of fields ([migrate.rs:124-131](../../crates/conductor-storage/src/migrate.rs)) | `client_installations` and `client_heartbeats` tables | `member_inventory` is keyed by `user_id` alone and cannot represent one member with two machines |
| `MemberPresence` domain type ([telemetry.rs](../../crates/conductor-domain/src/telemetry.rs)) | `PUT /api/v1/client/inventory` | `members_online` reads a table that is never written, so it is always zero ([dashboard.rs](../../crates/conductor-storage/src/repos/dashboard.rs)) |
| | Installation health views | |

## 4. Acceptance criteria

| ID | Criterion |
|---|---|
| AC-1 | `PUT /api/v1/client/inventory` accepts the authenticated project identifier, EvoFlux version, operating system, installation identifier, active workspace identifier, and project-attributed synchronized Agents, standalone Skills and Plugins with desired/applied resource version, release channel, artifact digest, local Plugin installation ID, enabled/trust state and synchronization errors |
| AC-2 | Repeated reports for the same `(project_id, installation_id)` update the existing record and never create duplicates; observed resource rows are unique by `(project_id, installation_id, resource_id)` |
| AC-3 | Heartbeats are recorded so that online and offline state can be derived from a configurable staleness threshold |
| AC-4 | `members_online` and every related figure reflect real reported state, satisfying V1 acceptance criterion 10 |
| AC-5 | Authorized users can list installations filtered by online state, EvoFlux version and member |
| AC-6 | Authorized users can identify installations running an outdated EvoFlux version |
| AC-7 | Authorized users can identify installations missing a required resource, or holding a version/channel other than the server-resolved effective Beta or Published assignment for that member |
| AC-8 | Synchronization failures reported by a client are visible with their error category |
| AC-9 | Workspace identifiers are normalized, never absolute local file paths, per [REQ-015](11-REQ-015-privacy-controls.md) |
| AC-10 | Inventory reporting requires the `sync_inventory` scope |
| AC-11 | Plugin observed state is one of `staged`, `trust_pending`, `active`, `update_pending`, `declined`, `incompatible`, `error` or `removed`, and it is compared with the assigned resource version/channel without treating `trust_pending` as synchronized |
| AC-12 | Inventory never includes Plugin package contents, Skill instructions, declared command arguments, environment/header values, credential values, mutable Plugin data or absolute local paths |
| AC-13 | The member can see the same plugin/resource inventory and synchronization errors for their own installations that an authorized administrator can see |
| AC-14 | Inventory accepts the release channel the server assigned but never accepts a client-provided beta audience or uses inventory to grant Beta; target membership remains server-owned under [REQ-008](10-REQ-008-resource-access-policy.md) |
| AC-15 | The endpoint derives project authorization from the connection token, requires every reported managed resource to match that project, and rejects the entire request transactionally on any cross-project resource ID or conflicting project ID |
| AC-16 | Installation and resource inventory views show the owning project name and retain the stable project ID in API/detail data; filters and desired-versus-observed joins always include project scope even while the V1 console exposes only one project |
| AC-17 | A project switch never relabels cached inventory: the previous project's last observed rows remain attributable to that project and the new heartbeat reports only resources active in the newly registered project namespace |

## 5. Out of scope

- Remote control of installations, such as forcing an update or restarting a client. Conductor
  distributes desired state and observes; it does not bypass local plugin trust or command execution.
- Hardware or system telemetry beyond operating system family.

## 6. Risks

| # | Risk | Severity | Mitigation |
|---|---|---|---|
| 1 | Inventory reveals private repository names or local paths | Medium | AC-9 and the collection level in [REQ-015](11-REQ-015-privacy-controls.md) |
| 2 | Heartbeat frequency generates excessive write load | Low | Separate low-frequency inventory from lightweight heartbeats |
| 3 | Online state flaps when a laptop sleeps | Low | AC-3 uses a configurable staleness threshold rather than an instantaneous flag |
| 4 | The existing `member_inventory` table is extended rather than replaced, preserving the one-machine assumption | Medium | Design must supersede it with `client_installations` |
| 5 | `trust_pending` is shown as healthy because the artifact was downloaded | Medium | AC-11 separates delivery, trust and active runtime state |
| 6 | Inventory leaks executable arguments or locally entered plugin credentials | High | AC-12 uses a typed metadata allowlist |
| 7 | A client claims Beta in inventory and is treated as authorized for it | High | AC-14 keeps release targeting server-owned and treats inventory as observation only |
| 8 | Inventory from one project is joined to same-name desired resources in another project | High | AC-2 and AC-15–AC-17 require project-scoped keys, authorization and UI attribution |

## 7. Open questions

- What staleness threshold defines offline? Fifteen minutes is proposed as a starting point.
- Should workspace identifiers be reported at all by default, given that even a hashed identifier can be
  correlated? Recommendation: report a salted hash by default, and the readable name only when the
  project enables it.

## History

| Date | Change | Author |
|---|---|---|
| 2026-08-09 | Created | |
| 2026-08-11 | Added desired-versus-observed Portable Agent Plugin inventory and privacy-safe trust state | Codex |
| 2026-08-11 | Added desired/applied release channel and protected Beta targeting from client inventory claims | Codex |
| 2026-08-11 | Added project-scoped inventory identity and cross-project rejection/isolation criteria | Codex |
| 2026-08-11 | Accepted into the coordinated governed-resource design by project-owner request | Codex |
