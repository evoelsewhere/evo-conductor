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

Status values: `Draft` awaiting decision, `Accepted` approved for design, `Rejected`, `Deferred`.

### Phase 0 — Foundation. Sequential; nothing else is safe to build on until these land

| Step | ID | Title | Priority | Depends on | Status |
|---|---|---|---|---|---|
| 1 | [REQ-020](requirement/01-REQ-020-automated-testing-ci.md) | Automated testing and CI | P0 | none | **Accepted · partial implementation** |
| 2 | [REQ-004](requirement/02-REQ-004-api-authorization.md) | API-enforced authorization | P0 | none | Draft |
| 3 | [REQ-001](requirement/03-REQ-001-versioned-migrations.md) | Versioned database migrations | P0 | 020 | Draft |
| 4 | [REQ-002](requirement/04-REQ-002-configuration-secret-protection.md) | Configuration secret protection | P0 | 001 | Draft |
| 5 | [REQ-018](requirement/05-REQ-018-audit-logging.md) | Audit logging | P0 | 001 | Draft |
| 6 | [REQ-003](requirement/06-REQ-003-server-project-separation.md) | Server and project configuration separation | P0 | 001, 020 | Draft |

Start with a thin slice of step 1: a test harness plus one three-role test against `GET /api/dashboard`.
That test fails on `user`, which is the evidence for step 2. Step 2 is then a four-line guard calling
`can_view_telemetry()`, a predicate that already exists and is never called.

### Phase 1 — Identity hardening

| Step | ID | Title | Priority | Depends on | Status |
|---|---|---|---|---|---|
| 7 | [REQ-005](requirement/07-REQ-005-member-lifecycle.md) | Member lifecycle and disablement | P0 | 004, 018 | Draft |
| 8 | [REQ-006](requirement/08-REQ-006-connection-tokens.md) | Connection tokens and scopes | P0 | 004, 005 | Draft |

### Phase 2 — Catalog

| Step | ID | Title | Priority | Depends on | Status |
|---|---|---|---|---|---|
| 9 | [REQ-007](requirement/09-REQ-007-resource-lifecycle.md) | Resource model, versioning and lifecycle | P0 | 001, 004, 018 | **Accepted · design draft** |
| 10 | [REQ-008](requirement/10-REQ-008-resource-access-policy.md) | Resource access policy | P0 | 004, 007 | **Accepted · coordinated design draft** |

### Phase 3 — Client integration. This is the vertical slice that proves the whole thesis

| Step | ID | Title | Priority | Depends on | Status |
|---|---|---|---|---|---|
| 11 | [REQ-015](requirement/11-REQ-015-privacy-controls.md) | Privacy controls and collection levels | P0 | 004 | Draft |
| 12 | [REQ-011](requirement/12-REQ-011-client-registration.md) | Client registration and connection | P0 | 001, 006, 015 | **Accepted · implementation in review** |
| 13 | [REQ-012](requirement/13-REQ-012-resource-sync-client.md) | Resource synchronization client | P0 | 007, 008, 011 | **Accepted · coordinated design draft** |
| 14 | [REQ-013](requirement/14-REQ-013-inventory-synchronization.md) | Inventory synchronization | P0 | 001, 011 | **Accepted · coordinated design draft** |

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
Implementation had already proceeded at the user's direction and is now in review in
[evo-conductor#2](https://github.com/evoelsewhere/evo-conductor/pull/2) and
[evoflux#4](https://github.com/evoelsewhere/evoflux/pull/4). [DES-011](design/12-DES-011-client-registration.md)
and its five [tasks](task/12-REQ-011-client-registration/) now record the as-built evidence and remaining
verification rather than claiming that design approval was satisfied retroactively.

### Phase 4 — Monitoring

| Step | ID | Title | Priority | Depends on | Status |
|---|---|---|---|---|---|
| 15 | [REQ-014](requirement/15-REQ-014-telemetry-ingestion.md) | Telemetry ingestion | P0 | 001, 011, 015 | **Accepted · partial implementation in review** |
| 16 | [REQ-021](requirement/16-REQ-021-console-i18n.md) | Console internationalization | P2 | none | Draft |
| 17 | [REQ-016](requirement/17-REQ-016-usage-aggregation-dashboards.md) | Usage aggregation and dashboards | P0 | 004, 013, 014, 015 | **Accepted · partial implementation in review** |

Step 16 is a decision point, not a dependency. REQ-021 has no prerequisites and can be done at any time,
but its cost grows with every screen added before it. Placed here it is cheapest, immediately before the
monitoring screens are built. Skip it if the team works in English.

**After step 17, run the V1 acceptance test from [requirements.md section 16](requirements.md).** All
sixteen criteria are achievable at that point.

### Phase 5 — Completion

| Step | ID | Title | Priority | Depends on | Status |
|---|---|---|---|---|---|
| 18 | [REQ-009](requirement/18-REQ-009-document-management.md) | Project document management | P1 | 001, 006, 007, 008 | Draft |
| 19 | [REQ-010](requirement/19-REQ-010-plugin-distribution-safety.md) | Plugin distribution safety | P0 | 007, 012 | **Accepted · coordinated design draft** |
| 20 | [REQ-017](requirement/20-REQ-017-cost-estimation.md) | Cost estimation and budget alerts | P1 | 014, 016 | Draft |
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
| Choose telemetry collection level L0 or L1; acceptance criterion 15 already rules out L2 | REQ-015, REQ-016 |
| Confirm whether Contributor may view individual member usage or only project totals; criterion 11 settles this for Admin only | REQ-004, REQ-016 |
| Confirm default connection-token lifetime | REQ-006 |
| Confirm whether PostgreSQL is required for the V1 acceptance run or only for production rollout | REQ-001 |

## Design register

| Step | ID | Requirement | Status |
|---|---|---|---|
| 01 | [DES-020](design/01-DES-020-automated-testing-ci.md) | REQ-020 | Draft · partial implementation reconciliation |
| 09 | [DES-007](design/09-DES-007-governed-resource-delivery.md) | REQ-007 plus REQ-008/010/012/013 | **Approved · implementation authorized** |
| 12 | [DES-011](design/12-DES-011-client-registration.md) | REQ-011 | Draft · as-built reconciliation |

## Task register

| ID | Layer | Title | Status |
|---|---|---|---|
| [TSK-020-01](task/01-REQ-020-automated-testing-ci/TSK-020-01-backend-test-harness.md) | BE | Build the backend test harness | In Review |
| [TSK-020-02](task/01-REQ-020-automated-testing-ci/TSK-020-02-authorization-suite.md) | BE | Write the authorization regression suite | Todo |
| [TSK-020-03](task/01-REQ-020-automated-testing-ci/TSK-020-03-frontend-unit-testing.md) | FE | Set up frontend unit testing and linting | Todo |
| [TSK-020-04](task/01-REQ-020-automated-testing-ci/TSK-020-04-frontend-e2e.md) | FE | Set up Playwright and one end-to-end flow | Todo |
| [TSK-020-05](task/01-REQ-020-automated-testing-ci/TSK-020-05-ci-pipeline.md) | Infra | Build the CI pipeline | Todo |
| [TSK-011-01](task/12-REQ-011-client-registration/TSK-011-01-installation-storage.md) | BE | Add installation registration storage | In Review |
| [TSK-011-02](task/12-REQ-011-client-registration/TSK-011-02-client-registration-api.md) | BE | Expose the client registration API | In Review |
| [TSK-011-03](task/12-REQ-011-client-registration/TSK-011-03-evoflux-connection-service.md) | EvoFlux | Implement EvoFlux connection service | In Review |
| [TSK-011-04](task/12-REQ-011-client-registration/TSK-011-04-evoflux-connection-ui.md) | EvoFlux FE | Build the EvoFlux connection experience | In Review |
| [TSK-011-05](task/12-REQ-011-client-registration/TSK-011-05-console-installations.md) | FE | Show installations in the Conductor console | In Review |
| [TSK-007-01](task/09-REQ-007-governed-resource-delivery/TSK-007-01-project-resource-schema.md) | BE | Add project-scoped resource schema and domain | Todo |
| [TSK-007-02](task/09-REQ-007-governed-resource-delivery/TSK-007-02-draft-import-validation.md) | BE | Build safe Draft import and validation | Todo |
| [TSK-007-03](task/09-REQ-007-governed-resource-delivery/TSK-007-03-plugin-artifact-store.md) | BE | Add immutable Plugin artifact storage | Todo |
| [TSK-007-04](task/09-REQ-007-governed-resource-delivery/TSK-007-04-release-versioning.md) | BE | Implement transactional release versioning | Todo |
| [TSK-007-05](task/09-REQ-007-governed-resource-delivery/TSK-007-05-effective-audience.md) | BE | Resolve access and Beta audience | Todo |
| [TSK-007-06](task/09-REQ-007-governed-resource-delivery/TSK-007-06-change-feed.md) | BE | Expose cursor changes and artifacts | Todo |
| [TSK-007-07](task/09-REQ-007-governed-resource-delivery/TSK-007-07-resource-studio-ui.md) | FE | Build Resource Studio and release UI | Todo |
| [TSK-007-08](task/09-REQ-007-governed-resource-delivery/TSK-007-08-evoflux-managed-state.md) | EvoFlux | Persist managed state and reconcile Agent/Skill | Todo |
| [TSK-007-09](task/09-REQ-007-governed-resource-delivery/TSK-007-09-evoflux-plugin-trust.md) | EvoFlux | Integrate Plugin staging and trust | Todo |
| [TSK-007-10](task/09-REQ-007-governed-resource-delivery/TSK-007-10-evoflux-sync-ui.md) | EvoFlux FE | Build sync, diff and trust UI | Todo |
| [TSK-007-11](task/09-REQ-007-governed-resource-delivery/TSK-007-11-inventory-ingestion.md) | BE | Ingest desired-versus-observed inventory | Todo |
| [TSK-007-12](task/09-REQ-007-governed-resource-delivery/TSK-007-12-cross-repo-proof.md) | Infra/QA | Prove cross-repo security and convergence | Todo |

## Implementation review snapshot — 2026-08-10

| Requirement | Delivered in open PRs | Remaining before requirement completion |
|---|---|---|
| REQ-011 | Registration, idempotency, heartbeat, OS credential vault, connection UI and member installations | Merge both PRs; PostgreSQL proof; connection Playwright coverage and DES-011 approval |
| REQ-014 | Privacy-safe event schema, scoped/idempotent batch ingestion, durable bounded outbox, indexes and retry after transient failure | Complete the specified event dimensions, report dropped events, classify permanent batch failures and load-test replay bursts |
| REQ-016 | Per-member overview, token/model charts, activity/request audit, tool analytics, date filters and member-token management | Add the aggregate table/job, project/team/tag/resource/Plugin views, server-time aggregation and a measured performance target |
