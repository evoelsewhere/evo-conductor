# REQ-011 — Client registration and connection

| | |
|---|---|
| ID | REQ-011 |
| Created | 2026-08-09 |
| Updated | 2026-08-10 |
| Status | Accepted (2026-08-10) — implementation in review |
| Priority | P0 |
| Build order | Step 12 of 23 |
| Spec section | [requirements.md sections 5 and 14](../requirements.md) |
| Source | Baseline specification sections 5 and 14 |
| Depends on | REQ-001, REQ-006 |
| Blocks | REQ-012, REQ-013, REQ-014, V1 acceptance criteria 3 and 4 |
| Repositories | `evo-conductor` and `evoflux` |
| Design | [DES-011](../design/12-DES-011-client-registration.md) — Draft as-built reconciliation; approval is still unrecorded |

## 1. Context

This is the point at which the two products meet. A temporary local V1 compatibility adapter now connects
EvoFlux to the resource-subscription endpoint, but it is not a registration protocol: it synthesizes a
local machine identifier, stores its credential through the current local store, and cannot return the
member/policy/bootstrap contract. The planned protocol below replaces that stopgap without treating a
read-only resource GET as enrolment.

Registration is separate from resource synchronization because an installation is a first-class entity: a
member may run EvoFlux on two machines, and monitoring must distinguish them.

## 2. Requirement

A member shall connect EvoFlux to a project by supplying the Conductor server URL, an `evc_` connection
token, and optionally a local workspace association. On connection, EvoFlux shall receive project
identity and branding, its own member identity and permissions, assigned resources, project policies, and
the telemetry and privacy configuration.

## 3. Implementation status

The project owner accepted this requirement on 2026-08-10. Implementation is open for review in
[evo-conductor#2](https://github.com/evoelsewhere/evo-conductor/pull/2) and
[evoflux#4](https://github.com/evoelsewhere/evoflux/pull/4). Acceptance does not imply that DES-011 is
approved or that the PRs have merged.

| Area | State | Evidence | Remaining verification |
|---|---|---|---|
| Installation storage and idempotent registration | Implemented in review | Conductor commits [`cec8571`](https://github.com/evoelsewhere/evo-conductor/commit/cec8571e66ed7b674d0e531a4f1cdfbcd3cca29e) and API contract tests | PostgreSQL migration run |
| Scoped registration and heartbeat APIs | Implemented in review | `client_registration.rs`: idempotency, invalid input/scope, owner scoping and revocation tests in Conductor PR #2 | Real cross-repo smoke after merge |
| EvoFlux credential, registration and heartbeat lifecycle | Implemented in review | EvoFlux commit [`4995fac3`](https://github.com/evoelsewhere/evoflux/commit/4995fac33469dd42b94f93636a3364a6e76b8194), keyring adapter and service tests | Packaged desktop/keyring smoke on each supported OS |
| EvoFlux connection UI and branding | Implemented in review | EvoFlux commit [`57b3f6b8`](https://github.com/evoelsewhere/evoflux/commit/57b3f6b85694e9a792a7d16badfa0b9ae7955fc8) and three component tests | Dedicated Playwright connect/revoke/disconnect flow |
| Conductor member installations | Implemented in review | Conductor commit [`ac01ad4`](https://github.com/evoelsewhere/evo-conductor/commit/ac01ad4c7329679b4e3364c87765b8c531b25a60) and privacy/authorization API test | Component/e2e proof for two installations in the console |

### Acceptance progress

| AC | State | Note |
|---|---|---|
| AC-1–AC-10, AC-12 | Implemented in review | Covered by Rust, pytest or component tests; final cross-repo smoke remains |
| AC-11 | Partially verified | Two installations and safe authorization are covered at the API boundary; console two-row e2e remains |

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
| 2026-08-10 | Corrected current V1 compatibility baseline after source review | Codex |
| 2026-08-10 | Linked pre-approval design and task planning requested for implementation preparation | Codex |
| 2026-08-10 | Reconciled implementation and test evidence from Conductor PR #2 and EvoFlux PR #4 | Codex |
| 2026-08-10 | Accepted by project owner | Project owner |
