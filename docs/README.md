# Evo Conductor — Documentation

## Workflow

```
REQ (Draft) --accepted by owner--> REQ (Accepted) --> DES --approved--> TSK --> code + test --> report
     |
     +--> Rejected / Deferred
```

A design is written only after its requirement is accepted. A task is written only after its design is
approved. No code is written without a task.

## Directory layout

| Directory | Contents | Created when |
|---|---|---|
| `base/` | Shared conventions and templates for all three document types | Exists; edit when conventions change |
| `requirement/` | `REQ-NNN-*.md`, business requirements | As soon as a requirement is identified |
| `design/` | `DES-NNN-*.md`, technical designs | After `REQ-NNN` is accepted |
| `task/` | `TSK-NNN-MM-*.md`, implementation and test tasks | After `DES-NNN` is approved |

## Foundation documents

| Document | Role |
|---|---|
| [base/BASE-CONVENTIONS.md](base/BASE-CONVENTIONS.md) | Read first. IDs, lifecycle, referencing rules, test tooling, privacy boundary |
| [base/TEMPLATE-REQUIREMENT.md](base/TEMPLATE-REQUIREMENT.md) | Template for every `REQ` |
| [base/TEMPLATE-DESIGN.md](base/TEMPLATE-DESIGN.md) | Template for every `DES` |
| [base/TEMPLATE-TASK.md](base/TEMPLATE-TASK.md) | Template for every `TSK`, including the mandatory test matrix |
| [requirements.md](requirements.md) | Source specification. Every `REQ` references back to a section of it |
| [architecture.md](architecture.md) | Current layering of the codebase |
| [resource-authoring-guide.md](resource-authoring-guide.md) | Normative Agent/Skill/Plugin structure, ZIP validation, editor and Draft/Beta/Publish behavior for Resource Studio |

## Requirement register — in build order

Follow this table top to bottom. `Step` is the implementation sequence, derived from the dependency
graph; the ID is a stable identifier and carries no priority meaning. Each requirement document repeats
its step in a `Build order` row.

Status values retain the owner's lifecycle decision (`Draft`, `Accepted`, `Rejected`, `Deferred`) and,
where useful, append the implementation state reconciled from source on 2026-08-14. Implementation does
not retroactively approve a requirement or design.

### Phase 0 — Foundation. Sequential; nothing else is safe to build on until these land

| Step | ID | Title | Priority | Depends on | Status |
|---|---|---|---|---|---|
| 1 | [REQ-020](requirement/01-REQ-020-automated-testing-ci.md) | Automated testing and CI | P0 | none | **Accepted · partial implementation** |
| 2 | [REQ-004](requirement/02-REQ-004-api-authorization.md) | API-enforced authorization | P0 | none | Draft · partial implementation |
| 3 | [REQ-001](requirement/03-REQ-001-versioned-migrations.md) | Versioned database migrations | P0 | 020 | Draft · schema bootstrap tested, not versioned |
| 4 | [REQ-002](requirement/04-REQ-002-configuration-secret-protection.md) | Configuration secret protection | P0 | 001 | Draft |
| 5 | [REQ-018](requirement/05-REQ-018-audit-logging.md) | Audit logging | P0 | 001 | Draft · resource-version events only |
| 6 | [REQ-003](requirement/06-REQ-003-server-project-separation.md) | Server and project configuration separation | P0 | 001, 020 | Draft · partial project scoping |

The backend harness now covers 94 Rust tests and focused authorization cases. The exhaustive three-role
route matrix remains open; specifically, `GET /api/dashboard` still authenticates without enforcing
`can_view_telemetry()`. Resource catalog visibility and explicit secret-scope validation have already
been hardened in current source.

### Phase 1 — Identity hardening

| Step | ID | Title | Priority | Depends on | Status |
|---|---|---|---|---|---|
| 7 | [REQ-005](requirement/07-REQ-005-member-lifecycle.md) | Member lifecycle and disablement | P0 | 004, 018 | Draft · partial implementation |
| 8 | [REQ-006](requirement/08-REQ-006-connection-tokens.md) | Connection tokens and scopes | P0 | 004, 005 | Draft · core scoped-token lifecycle implemented |

### Phase 2 — Catalog

| Step | ID | Title | Priority | Depends on | Status |
|---|---|---|---|---|---|
| 9 | [REQ-007](requirement/09-REQ-007-resource-lifecycle.md) | Resource model, versioning and lifecycle | P0 | 001, 004, 018 | **Accepted · substantial implementation** |
| 10 | [REQ-008](requirement/10-REQ-008-resource-access-policy.md) | Resource access policy | P0 | 004, 007 | **Accepted · allow-only V1 implemented; preview gaps** |

### Phase 3 — Client integration. This is the vertical slice that proves the whole thesis

| Step | ID | Title | Priority | Depends on | Status |
|---|---|---|---|---|---|
| 11 | [REQ-015](requirement/11-REQ-015-privacy-controls.md) | Privacy controls and collection levels | P0 | 004 | Draft · privacy boundary partially implemented |
| 12 | [REQ-011](requirement/12-REQ-011-client-registration.md) | Client registration and connection | P0 | 001, 006, 015 | **Accepted · merged with verification gaps** |
| 13 | [REQ-012](requirement/13-REQ-012-resource-sync-client.md) | Resource synchronization client | P0 | 007, 008, 011 | **Accepted · cursor client implemented; smart-fetch pending** |
| 14 | [REQ-013](requirement/14-REQ-013-inventory-synchronization.md) | Inventory synchronization | P0 | 001, 011 | **Accepted · core inventory implemented** |

REQ-015 precedes REQ-011 because registration must return the active collection level
(REQ-011 AC-5), which cannot be returned before it is defined.

### Owner-selected next epic — governed resource delivery

On 2026-08-11 the project owner selected Conductor-to-EvoFlux delivery of agents, standalone Skills and
Portable Agent Plugins as the next product slice. The governing requirements are REQ-007, REQ-008,
REQ-012 and REQ-010. REQ-012 may deliver and stage an executable package, but REQ-010's local trust gate
must be included before the package can be reported active. REQ-013 then records desired-versus-observed
version and trust state. The slice now also includes EvoFlux-compatible Agent/Skill/Plugin guides,
template or safe ZIP-to-Draft creation, Monaco editing, structured validation and an explicit-member Beta
channel with Published fallback. This priority note does not bypass the requirement acceptance and design
approval gates.

### Lifecycle exception and implementation reconciliation

REQ-011 was accepted by the project owner on 2026-08-10; DES-011 approval is still not recorded.
Implementation from the former Conductor PR #2 and EvoFlux PR #4 is now present in the current source.
[DES-011](design/12-DES-011-client-registration.md) and its five
[tasks](task/12-REQ-011-client-registration/) record the as-built evidence and remaining PostgreSQL,
packaged-OS and browser verification without claiming that design approval was satisfied retroactively.

### Phase 4 — Monitoring

| Step | ID | Title | Priority | Depends on | Status |
|---|---|---|---|---|---|
| 15 | [REQ-014](requirement/15-REQ-014-telemetry-ingestion.md) | Telemetry ingestion | P0 | 001, 011, 015 | **Accepted · ingestion/attribution implemented; gaps remain** |
| 16 | [REQ-021](requirement/16-REQ-021-console-i18n.md) | Console internationalization | P2 | none | Draft |
| 17 | [REQ-016](requirement/17-REQ-016-usage-aggregation-dashboards.md) | Usage aggregation and dashboards | P0 | 004, 013, 014, 015 | **Accepted · Analytics Studio implemented; aggregate layer open** |

Step 16 is a decision point, not a dependency. REQ-021 has no prerequisites and can be done at any time,
but its cost grows with every screen added before it. Placed here it is cheapest, immediately before the
monitoring screens are built. Skip it if the team works in English.

**Run the V1 acceptance test from [requirements.md section 16](requirements.md) after the remaining
foundation/security gaps are closed.** The current source implements most product-path criteria but does
not yet satisfy all sixteen, notably general audit, migration/secret hardening and complete automated
cross-repository proof.

### Phase 5 — Completion

| Step | ID | Title | Priority | Depends on | Status |
|---|---|---|---|---|---|
| 18 | [REQ-009](requirement/18-REQ-009-document-management.md) | Project document management | P1 | 001, 006, 007, 008 | Draft |
| 19 | [REQ-010](requirement/19-REQ-010-plugin-distribution-safety.md) | Plugin distribution safety | P0 | 007, 012 | **Accepted · delivery/trust implemented; hardening remains** |
| 20 | [REQ-017](requirement/20-REQ-017-cost-estimation.md) | Cost estimation and budget alerts | P1 | 014, 016 | Draft · client estimates implemented; pricing/alerts open |
| 21 | [REQ-019](requirement/21-REQ-019-data-retention.md) | Data retention | P1 | 014, 016, 018 | Draft |
| 22 | [REQ-022](requirement/22-REQ-022-model-access-policy.md) | Model access policy | P2 | 007, 012, 016 | Draft |
| 23 | [REQ-023](requirement/23-REQ-023-ai-gateway.md) | AI gateway | Deferred | 002, 004, 014 | Draft |

Step 23 is deferred by [requirements.md section 10](requirements.md) and is listed for completeness. Do
not build toward it without that requirement being accepted first.

### Reordering rule

If you change the order, change the dependency rows too. A step may move earlier only if every
requirement it depends on still precedes it.

## Decisions required before design can start

| Question | Affects |
|---|---|
| Confirm one deployment per project for V1, while still preparing the schema for multi-project | REQ-003 |
| Confirm rollout defaults for L0/L1/L2. Current source defines L0 off, L1 counters and L2 extended privacy-safe attribution; no level captures prompt/response content | REQ-015, REQ-016 |
| Confirm whether Contributor may view individual member usage or only project totals; criterion 11 settles this for Admin only | REQ-004, REQ-016 |
| Confirm default connection-token lifetime | REQ-006 |
| Confirm whether PostgreSQL is required for the V1 acceptance run or only for production rollout | REQ-001 |

## Design register

| Step | ID | Requirement | Status |
|---|---|---|---|
| 01 | [DES-020](design/01-DES-020-automated-testing-ci.md) | REQ-020 | Draft · backend harness implemented; CI gaps |
| 09 | [DES-007](design/09-DES-007-governed-resource-delivery.md) | REQ-007 plus REQ-008/010/012/013 | **Approved · implementation reconciled** |
| 12 | [DES-011](design/12-DES-011-client-registration.md) | REQ-011 | Draft · as-built implementation reconciliation |

## Task register

| ID | Layer | Title | Status |
|---|---|---|---|
| [TSK-020-01](task/01-REQ-020-automated-testing-ci/TSK-020-01-backend-test-harness.md) | BE | Build the backend test harness | Implemented; reporting gaps |
| [TSK-020-02](task/01-REQ-020-automated-testing-ci/TSK-020-02-authorization-suite.md) | BE | Write the authorization regression suite | Partial |
| [TSK-020-03](task/01-REQ-020-automated-testing-ci/TSK-020-03-frontend-unit-testing.md) | FE | Set up frontend unit testing and linting | Todo |
| [TSK-020-04](task/01-REQ-020-automated-testing-ci/TSK-020-04-frontend-e2e.md) | FE | Set up Playwright and one end-to-end flow | Todo |
| [TSK-020-05](task/01-REQ-020-automated-testing-ci/TSK-020-05-ci-pipeline.md) | Infra | Build the CI pipeline | Todo |
| [TSK-011-01](task/12-REQ-011-client-registration/TSK-011-01-installation-storage.md) | BE | Add installation registration storage | Implemented; PostgreSQL proof open |
| [TSK-011-02](task/12-REQ-011-client-registration/TSK-011-02-client-registration-api.md) | BE | Expose the client registration API | Implemented |
| [TSK-011-03](task/12-REQ-011-client-registration/TSK-011-03-evoflux-connection-service.md) | EvoFlux | Implement EvoFlux connection service | Implemented; packaged smoke open |
| [TSK-011-04](task/12-REQ-011-client-registration/TSK-011-04-evoflux-connection-ui.md) | EvoFlux FE | Build the EvoFlux connection experience | Implemented; Playwright open |
| [TSK-011-05](task/12-REQ-011-client-registration/TSK-011-05-console-installations.md) | FE | Show installations in the Conductor console | Implemented; UI tests open |
| [TSK-007-01](task/09-REQ-007-governed-resource-delivery/TSK-007-01-project-resource-schema.md) | BE | Add project-scoped resource schema and domain | Implemented; foundation gaps |
| [TSK-007-02](task/09-REQ-007-governed-resource-delivery/TSK-007-02-draft-import-validation.md) | BE | Build safe Draft import and validation | Implemented; shared fixtures open |
| [TSK-007-03](task/09-REQ-007-governed-resource-delivery/TSK-007-03-plugin-artifact-store.md) | BE | Add immutable Plugin artifact storage | Implemented; streaming proof open |
| [TSK-007-04](task/09-REQ-007-governed-resource-delivery/TSK-007-04-release-versioning.md) | BE | Implement transactional release versioning | Implemented; audit/PostgreSQL gaps |
| [TSK-007-05](task/09-REQ-007-governed-resource-delivery/TSK-007-05-effective-audience.md) | BE | Resolve access and Beta audience | Partial |
| [TSK-007-06](task/09-REQ-007-governed-resource-delivery/TSK-007-06-change-feed.md) | BE | Expose cursor changes and artifacts | Implemented on Conductor |
| [TSK-007-07](task/09-REQ-007-governed-resource-delivery/TSK-007-07-resource-studio-ui.md) | FE | Build Resource Studio and release UI | Implemented; UI tests open |
| [TSK-007-08](task/09-REQ-007-governed-resource-delivery/TSK-007-08-evoflux-managed-state.md) | EvoFlux | Persist managed state and reconcile Agent/Skill | Implemented with cursor; smart-fetch open |
| [TSK-007-09](task/09-REQ-007-governed-resource-delivery/TSK-007-09-evoflux-plugin-trust.md) | EvoFlux | Integrate Plugin staging and trust | Implemented; packaged E2E open |
| [TSK-007-10](task/09-REQ-007-governed-resource-delivery/TSK-007-10-evoflux-sync-ui.md) | EvoFlux FE | Build sync, diff and trust UI | Implemented; Playwright open |
| [TSK-007-11](task/09-REQ-007-governed-resource-delivery/TSK-007-11-inventory-ingestion.md) | BE | Ingest desired-versus-observed inventory | Core implemented; fleet views partial |
| [TSK-007-12](task/09-REQ-007-governed-resource-delivery/TSK-007-12-cross-repo-proof.md) | Infra/QA | Prove cross-repo security and convergence | Partial |

## Source reconciliation snapshot — 2026-08-14

| Requirement | Present in current source | Remaining before requirement completion |
|---|---|---|
| REQ-007/008/010/012/013 | Project-scoped Resource Studio, immutable object-backed releases, allow-only audience/Beta resolution, cursor and smart-fetch APIs, EvoFlux reconciliation/plugin trust and core inventory | Smart-fetch EvoFlux checkout, publication/audit/security hardening, full fleet views and packaged two-repository E2E |
| REQ-011 | Registration, idempotency, heartbeat, OS credential vault, connection UI and member installations | PostgreSQL proof, packaged OS/restart smoke, browser coverage and formal DES-011 approval |
| REQ-014/015 | Privacy-safe event schema, scoped/idempotent ingestion, durable bounded outbox and managed-resource attribution | L1/L2 client field differentiation, dropped-event visibility, permanent-4xx handling and complete run/session/cost dimensions |
| REQ-016/017 | Analytics Studio, saved views, portfolio/member/resource drill-downs, client estimated cost and unpriced-call reporting | Aggregate table/job, server pricing/versioning, budget alerts and current-scale performance proof |
| REQ-001/002/004/005/006/018 | Tested schema bootstrap, broad route guards, member disablement and explicit scoped/revocable tokens | Versioned migrations, OIDC secret encryption, dashboard guard, token expiry/read-document policy and general audit log |
