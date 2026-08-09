# REQ-011 — Client registration and connection

| | |
|---|---|
| ID | REQ-011 |
| Created | 2026-08-09 |
| Updated | 2026-08-09 |
| Status | Draft |
| Priority | P0 |
| Build order | Step 12 of 23 |
| Spec section | [requirements.md sections 5 and 14](../requirements.md) |
| Source | Baseline specification sections 5 and 14 |
| Depends on | REQ-001, REQ-006 |
| Blocks | REQ-012, REQ-013, REQ-014, V1 acceptance criteria 3 and 4 |
| Repositories | `evo-conductor` and `evoflux` |
| Design | Not created; requires acceptance |

## 1. Context

This is the point at which the two products meet, and none of it exists yet. Searching the entire
`evoflux` repository for the string `conductor` across Python, TypeScript, Rust and Markdown returns zero
matches. The integration currently exists only on the server side.

Registration is separate from resource synchronization because an installation is a first-class entity: a
member may run EvoFlux on two machines, and monitoring must distinguish them.

## 2. Requirement

A member shall connect EvoFlux to a project by supplying the Conductor server URL, an `evc_` connection
token, and optionally a local workspace association. On connection, EvoFlux shall receive project
identity and branding, its own member identity and permissions, assigned resources, project policies, and
the telemetry and privacy configuration.

## 3. Implementation status

| Implemented | Missing |
|---|---|
| Token authentication path on one endpoint ([resources.rs:17-51](../../crates/conductor-server/src/http/routes/resources.rs)) | `POST /api/v1/client/register`, `POST /api/v1/client/heartbeat` |
| Project branding endpoint for browser sessions ([settings.rs](../../crates/conductor-server/src/http/routes/settings.rs)) | `client_installations` table |
| | Any EvoFlux-side client, settings screen or credential storage |
| | A token-authenticated endpoint returning identity, policy and privacy configuration |

## 4. Acceptance criteria

| ID | Criterion |
|---|---|
| AC-1 | EvoFlux provides a settings screen accepting a Conductor URL and an `evc_` token, and validates them immediately |
| AC-2 | `POST /api/v1/client/register` records an installation with a stable installation identifier, EvoFlux version and operating system, and returns project identity and branding |
| AC-3 | Registration is idempotent: re-registering the same installation updates the existing record rather than creating a duplicate |
| AC-4 | The registration response includes the member's identity, primary role, sub-roles and tags |
| AC-5 | The registration response includes the active telemetry and privacy configuration, including the collection level from [REQ-015](11-REQ-015-privacy-controls.md) |
| AC-6 | EvoFlux displays the connected project name and logo, satisfying V1 acceptance criterion 4 |
| AC-7 | The token is stored in the operating system credential store, per [REQ-006](08-REQ-006-connection-tokens.md) AC-9 |
| AC-8 | `POST /api/v1/client/heartbeat` updates last-seen state and is safe to call repeatedly |
| AC-9 | An invalid, expired or revoked token produces a clear message in EvoFlux and stops the connection attempt without an unbounded retry loop |
| AC-10 | Disconnecting removes the stored token and stops all further communication |
| AC-11 | One member with two installations appears as two installations in the console, correctly attributed to the same member |
| AC-12 | EvoFlux sends a heartbeat on a configurable interval while running; the default interval is stated, and the schedule survives a restart |

## 5. Out of scope

- Resource download, covered by [REQ-012](13-REQ-012-resource-sync-client.md).
- Inventory contents, covered by [REQ-013](14-REQ-013-inventory-synchronization.md).
- Telemetry upload, covered by [REQ-014](15-REQ-014-telemetry-ingestion.md).
- Connecting one EvoFlux installation to more than one Conductor project simultaneously.

## 6. Risks

| # | Risk | Severity | Mitigation |
|---|---|---|---|
| 1 | Installation identity is derived from something unstable, producing duplicate records after an upgrade or reinstall | Medium | Define the identifier explicitly in the design and persist it locally |
| 2 | A failed connection blocks EvoFlux from starting | High | Connection is additive; EvoFlux must work fully when Conductor is unreachable |
| 3 | Token accidentally logged during connection troubleshooting | High | [REQ-006](08-REQ-006-connection-tokens.md) AC-9 |

## 7. Open questions

- How is the installation identifier derived: a locally generated UUID persisted in the EvoFlux
  configuration directory, or a value derived from the host? A persisted UUID is recommended, since it is
  stable across upgrades and reveals nothing about the machine.
- Should one installation be able to connect to several projects at once? Recommendation: no for V1,
  consistent with one deployment per project.

## History

| Date | Change | Author |
|---|---|---|
| 2026-08-09 | Created | |
