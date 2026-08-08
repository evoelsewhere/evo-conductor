# REQ-013 — Inventory synchronization

| | |
|---|---|
| ID | REQ-013 |
| Created | 2026-08-09 |
| Updated | 2026-08-09 |
| Status | Draft |
| Priority | P0 |
| Spec section | [requirements.md section 8](../requirements.md) |
| Source | Baseline specification section 8 |
| Depends on | REQ-001, REQ-011 |
| Blocks | REQ-016, V1 acceptance criteria 6 and 8 |
| Repositories | `evo-conductor` and `evoflux` |
| Design | Not created; requires acceptance |

## 1. Context

Inventory answers the operational questions that usage metrics cannot: who is actually connected, who is
running an outdated version, whose synchronization is failing, and where a resource has not yet landed.

The current dashboard already claims to answer the first of these and cannot. It computes
`members_online` from a table that is never written, so the figure is permanently zero. A monitoring
screen displaying a fabricated number is worse than one that displays nothing.

## 2. Requirement

EvoFlux shall periodically report its local project inventory, and Conductor shall accept those reports
idempotently and surface installation health to authorized users.

## 3. Implementation status

| Implemented | Missing | Incorrect |
|---|---|---|
| `member_inventory` table with a small subset of fields ([migrate.rs:124-131](../../crates/conductor-storage/src/migrate.rs)) | `client_installations` and `client_heartbeats` tables | `member_inventory` is keyed by `user_id` alone and cannot represent one member with two machines |
| `MemberPresence` domain type ([telemetry.rs](../../crates/conductor-domain/src/telemetry.rs)) | `PUT /api/v1/client/inventory` | `members_online` reads a table that is never written, so it is always zero ([dashboard.rs](../../crates/conductor-storage/src/repos/dashboard.rs)) |
| | Installation health views | |

## 4. Acceptance criteria

| ID | Criterion |
|---|---|
| AC-1 | `PUT /api/v1/client/inventory` accepts EvoFlux version, operating system, installation identifier, active workspace identifier, synchronized agents, skills, MCP servers, workflow versions and synchronization errors |
| AC-2 | Repeated reports for the same installation update the existing record and never create duplicates |
| AC-3 | Heartbeats are recorded so that online and offline state can be derived from a configurable staleness threshold |
| AC-4 | `members_online` and every related figure reflect real reported state, satisfying V1 acceptance criterion 8 |
| AC-5 | Authorized users can list installations filtered by online state, EvoFlux version and member |
| AC-6 | Authorized users can identify installations running an outdated EvoFlux version |
| AC-7 | Authorized users can identify installations missing a required resource, or holding a version other than the current published one |
| AC-8 | Synchronization failures reported by a client are visible with their error category |
| AC-9 | Workspace identifiers are normalized, never absolute local file paths, per [REQ-015](REQ-015-privacy-controls.md) |
| AC-10 | Inventory reporting requires the `sync_inventory` scope |

## 5. Out of scope

- Remote control of installations, such as forcing an update or restarting a client. Conductor
  distributes and observes; it does not command.
- Hardware or system telemetry beyond operating system family.

## 6. Risks

| # | Risk | Severity | Mitigation |
|---|---|---|---|
| 1 | Inventory reveals private repository names or local paths | Medium | AC-9 and the collection level in [REQ-015](REQ-015-privacy-controls.md) |
| 2 | Heartbeat frequency generates excessive write load | Low | Separate low-frequency inventory from lightweight heartbeats |
| 3 | Online state flaps when a laptop sleeps | Low | AC-3 uses a configurable staleness threshold rather than an instantaneous flag |
| 4 | The existing `member_inventory` table is extended rather than replaced, preserving the one-machine assumption | Medium | Design must supersede it with `client_installations` |

## 7. Open questions

- What staleness threshold defines offline? Fifteen minutes is proposed as a starting point.
- Should workspace identifiers be reported at all by default, given that even a hashed identifier can be
  correlated? Recommendation: report a salted hash by default, and the readable name only when the
  project enables it.

## History

| Date | Change | Author |
|---|---|---|
| 2026-08-09 | Created | |
