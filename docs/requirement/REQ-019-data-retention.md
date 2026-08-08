# REQ-019 — Data retention

| | |
|---|---|
| ID | REQ-019 |
| Created | 2026-08-09 |
| Updated | 2026-08-09 |
| Status | Draft |
| Priority | P1 |
| Spec section | [requirements.md sections 9 and 10](../requirements.md) |
| Source | Baseline specification sections 9 and 10, which require configurable retention |
| Depends on | REQ-014, REQ-016, REQ-018 |
| Blocks | none |
| Repositories | `evo-conductor` |
| Design | Not created; requires acceptance |

## 1. Context

The specification requires configurable retention for telemetry and states that any detailed content
collection must be covered by a retention period. Retention is also the practical control that keeps the
raw event table from growing without limit.

Different data classes warrant different periods, and conflating them either discards useful history or
keeps sensitive detail far longer than necessary.

## 2. Requirement

Conductor shall apply configurable retention per data class, shall delete expired data automatically, and
shall record both the policy and its enforcement.

## 3. Implementation status

| Implemented | Missing |
|---|---|
| Nothing | Retention configuration, the deletion job, and reporting of what was deleted |

## 4. Acceptance criteria

| ID | Criterion |
|---|---|
| AC-1 | Retention is configurable per data class: raw telemetry events, aggregated usage, inventory and heartbeat history, audit records, and any L2 content if enabled |
| AC-2 | Defaults are raw telemetry thirty to ninety days, aggregates retained long term, audit records retained longer than telemetry |
| AC-3 | Expired data is deleted automatically by a scheduled job |
| AC-4 | Deleting raw events never alters previously computed aggregates |
| AC-5 | Each deletion run records what was deleted and how much, without recording the deleted content |
| AC-6 | The active retention policy is visible to every member, per [REQ-015](REQ-015-privacy-controls.md) AC-1 |
| AC-7 | Changing the policy is recorded in the audit log ([REQ-018](REQ-018-audit-logging.md)) |
| AC-8 | Shortening a retention period does not delete data retroactively without an explicit confirmation |
| AC-9 | Records belonging to a disabled member are retained according to policy rather than deleted on disablement, per [REQ-005](REQ-005-member-lifecycle.md) AC-5 |

## 5. Out of scope

- Legal hold and litigation preservation.
- Right-to-erasure workflows for individuals. Reconsider at P2 if a legal obligation applies.
- Archiving expired data to cold storage before deletion.

## 6. Risks

| # | Risk | Severity | Mitigation |
|---|---|---|---|
| 1 | A misconfigured policy destroys history that was still needed | High | AC-8 requires confirmation before retroactive deletion |
| 2 | Deleting raw events distorts historical charts | Medium | AC-4 keeps aggregates independent of raw data |
| 3 | The deletion job silently stops running and the table grows unnoticed | Medium | AC-5 makes each run observable |

## 7. Open questions

- What retention period applies to audit records? One to two years is typical for administrative logs.
- Should aggregates be retained indefinitely, or trimmed after a fixed number of years?

## History

| Date | Change | Author |
|---|---|---|
| 2026-08-09 | Created | |
