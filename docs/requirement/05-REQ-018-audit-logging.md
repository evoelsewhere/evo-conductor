# REQ-018 — Audit logging

| | |
|---|---|
| ID | REQ-018 |
| Created | 2026-08-09 |
| Updated | 2026-08-14 |
| Status | Draft — partial resource-version event history only |
| Priority | P0 |
| Build order | Step 5 of 23 |
| Spec section | [requirements.md section 13](../requirements.md) |
| Source | Baseline specification section 13 |
| Depends on | REQ-001 |
| Blocks | REQ-005, REQ-007, REQ-010, V1 acceptance criterion 16 |
| Repositories | `evo-conductor` |
| Design | Not created; requires acceptance |

## 1. Context

Conductor holds the team's accounts, tokens and permissions, and is about to push configuration onto
other people's machines. There is currently no audit table anywhere in the migration.

This is a prerequisite for resource publishing rather than a follow-up to it. Publishing a prompt that
changes agent behaviour across the whole team, with no record of who changed it or what it replaced, is
not a controlled process.

## 2. Requirement

Conductor shall record every security-relevant and administrative action in an append-only log
containing actor, action, target, timestamp, result, a safe change summary and a request correlation
identifier. Secrets shall never appear in the log.

## 3. Implementation status

| Implemented | Missing |
|---|---|
| `resource_version_events` durably records release/deprecate/restore lifecycle facts for immutable versions | A general append-only `audit_events` table covering every administrative surface |
| Resource/version history and member invitation/approval columns preserve isolated provenance facts | Save, access, Beta target, member, token, settings, SSO, policy and denial events |
| `tracing` provides operational logs | Before/after summaries, request correlation, source address, filtering/export and an audit console |
| Cross-member usage access is server-authorized | Auditing the act of inspecting another member's usage |

## 4. Acceptance criteria

| ID | Criterion |
|---|---|
| AC-1 | An `audit_events` table stores id, project, actor, action, target type, target id, timestamp, result, safe change summary, request correlation id and source address |
| AC-2 | The log is append-only; no API path updates or deletes a record, including for Admin |
| AC-3 | Coverage includes member created, approved, updated, enabled and disabled; role and tag assignment changes; token created and revoked; project settings changed; SSO configuration changed; resource draft imported/saved, validation warnings acknowledged, Beta released/retargeted/promoted/retired, Published, updated, deprecated and archived; retention and telemetry policy changed |
| AC-4 | Update actions record a before and after summary |
| AC-5 | Secrets and raw passwords never appear; a redaction field list is enforced and tested |
| AC-6 | Admin can view and filter by actor, action type, target and date range |
| AC-7 | Records can be exported as CSV |
| AC-8 | Actions rejected for insufficient permission are recorded, since repeated rejections indicate permission probing |
| AC-9 | Viewing another member's individual usage data or request detail is itself recorded with viewer, target member, timestamp, route and safe filter summary; the audit record never copies the usage values or any work content |
| AC-10 | A failure to write an audit record does not silently succeed: it raises a visible error and does not roll back the primary action |
| AC-11 | The correlation identifier links an audit record to the originating request across log lines |

## 5. Out of scope

- Usage auditing, meaning which member used which Agent/Skill/Plugin/model/tool, when and how much, is a
  different concern covered by [REQ-014](15-REQ-014-telemetry-ingestion.md),
  [REQ-016](17-REQ-016-usage-aggregation-dashboards.md) and
  [REQ-015](11-REQ-015-privacy-controls.md). This append-only log records access to that view, not a
  duplicate copy of every usage event.
- Forwarding to an external SIEM. Reconsider at P2.
- Cryptographic tamper-evidence. Reconsider at P2.

## 6. Risks

| # | Risk | Severity | Mitigation |
|---|---|---|---|
| 1 | Secrets leak through the before or after summary | High | AC-5 with an enforced redaction list and a test |
| 2 | Recording is added per handler and forgotten for new endpoints | Medium | The design should evaluate a shared recording layer rather than manual calls |
| 3 | Audit writes slow down or break primary actions | Medium | AC-10 defines the failure behaviour explicitly |
| 4 | The table grows without bound | Low | Administrative actions are infrequent; apply retention if measurement says otherwise |

## 7. Open questions

- Should the source IP address be recorded? Recommended: yes. It is administrative metadata rather than
  work content, and it is the field most often needed during an investigation.
- What retention applies to audit records? These are usually kept longer than telemetry; see
  [REQ-019](21-REQ-019-data-retention.md).

## History

| Date | Change | Author |
|---|---|---|
| 2026-08-09 | Created | |
| 2026-08-11 | Added Resource Studio import/validation and Beta-channel actions to mandatory audit coverage | Codex |
| 2026-08-11 | Clarified usage analytics versus administrative audit and required privacy-safe logging of cross-member drill-down reads | Codex |
| 2026-08-14 | Recorded the implemented resource-version event ledger while keeping the project-wide audit requirement open | Codex |
