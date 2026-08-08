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

## Requirement register

Status values: `Draft` awaiting decision, `Accepted` approved for design, `Rejected`, `Deferred`.

### Foundation — required before the V1 acceptance run

| ID | Title | Priority | Spec section | Status |
|---|---|---|---|---|
| [REQ-001](requirement/REQ-001-versioned-migrations.md) | Versioned database migrations | P0 | 15 | Draft |
| [REQ-002](requirement/REQ-002-configuration-secret-protection.md) | Configuration secret protection | P0 | 15 | Draft |
| [REQ-003](requirement/REQ-003-server-project-separation.md) | Server and project configuration separation | P0 | 1 | Draft |
| [REQ-004](requirement/REQ-004-api-authorization.md) | API-enforced authorization | P0 | 3, 7 | Draft |

### Identity and access

| ID | Title | Priority | Spec section | Status |
|---|---|---|---|---|
| [REQ-005](requirement/REQ-005-member-lifecycle.md) | Member lifecycle and disablement | P0 | 4 | Draft |
| [REQ-006](requirement/REQ-006-connection-tokens.md) | Connection tokens and scopes | P0 | 5 | Draft |

### Resources and documents

| ID | Title | Priority | Spec section | Status |
|---|---|---|---|---|
| [REQ-007](requirement/REQ-007-resource-lifecycle.md) | Resource model, versioning and lifecycle | P0 | 6 | Draft |
| [REQ-008](requirement/REQ-008-resource-access-policy.md) | Resource access policy | P0 | 7 | Draft |
| [REQ-009](requirement/REQ-009-document-management.md) | Project document management | P1 | 12 | Draft |
| [REQ-010](requirement/REQ-010-mcp-distribution-safety.md) | MCP distribution safety | P1 | 6 | Draft |

### EvoFlux client integration

| ID | Title | Priority | Spec section | Status |
|---|---|---|---|---|
| [REQ-011](requirement/REQ-011-client-registration.md) | Client registration and connection | P0 | 5, 14 | Draft |
| [REQ-012](requirement/REQ-012-resource-sync-client.md) | Resource synchronization client | P0 | 6, 12 | Draft |
| [REQ-013](requirement/REQ-013-inventory-synchronization.md) | Inventory synchronization | P0 | 8 | Draft |

### Monitoring

| ID | Title | Priority | Spec section | Status |
|---|---|---|---|---|
| [REQ-014](requirement/REQ-014-telemetry-ingestion.md) | Telemetry ingestion | P0 | 9 | Draft |
| [REQ-015](requirement/REQ-015-privacy-controls.md) | Privacy controls and collection levels | P0 | 10 | Draft |
| [REQ-016](requirement/REQ-016-usage-aggregation-dashboards.md) | Usage aggregation and dashboards | P0 | 11 | Draft |
| [REQ-017](requirement/REQ-017-cost-estimation.md) | Cost estimation and budget alerts | P1 | 11 | Draft |
| [REQ-019](requirement/REQ-019-data-retention.md) | Data retention | P1 | 9, 10 | Draft |

### Governance

| ID | Title | Priority | Spec section | Status |
|---|---|---|---|---|
| [REQ-018](requirement/REQ-018-audit-logging.md) | Audit logging | P0 | 13 | Draft |

### Platform quality

| ID | Title | Priority | Spec section | Status |
|---|---|---|---|---|
| [REQ-020](requirement/REQ-020-automated-testing-ci.md) | Automated testing and CI | P0 | 16 | Draft |
| [REQ-021](requirement/REQ-021-console-i18n.md) | Console internationalization | P2 | Addition | Draft |
| [REQ-022](requirement/REQ-022-model-access-policy.md) | Model access policy | P2 | Addition | Draft |
| [REQ-023](requirement/REQ-023-ai-gateway.md) | AI gateway, deferred | Deferred | Addition | Draft |

## Decisions required before design can start

| Question | Affects |
|---|---|
| Confirm one deployment per project for V1, while still preparing the schema for multi-project | REQ-003 |
| Choose telemetry collection level L0, L1 or L2 | REQ-015, REQ-016 |
| Confirm whether Contributor may view individual member usage or only project totals | REQ-004, REQ-016 |
| Confirm default connection-token lifetime | REQ-006 |
| Confirm whether PostgreSQL is required for the V1 acceptance run or only for production rollout | REQ-001 |

## Design register

None. Created once a requirement is accepted. See [design/README.md](design/README.md).

## Task register

None. Created once a design is approved. See [task/README.md](task/README.md).
