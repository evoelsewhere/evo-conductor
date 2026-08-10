# TSK-007-05 — Resolve access, Beta audience and effective versions

| | |
|---|---|
| ID | TSK-007-05 |
| Created / updated | 2026-08-11 |
| Status | Todo |
| Layer | BE (Rust) |
| Requirement | [REQ-008](../../requirement/10-REQ-008-resource-access-policy.md) |
| Design | [DES-007 sections 5.2 and 6](../../design/09-DES-007-governed-resource-delivery.md) |
| Covers | REQ-008 AC-1–AC-14; REQ-007 AC-12, AC-26–AC-28 |
| Depends on | TSK-007-01, TSK-007-04 |
| Estimate | 3d |
| Branch | `feat/REQ-007-governed-resource-delivery` |

## Goal

Implement one server-side resolver used by catalog preview, change feed, metadata and artifact download so
Beta narrows normal access and direct version IDs cannot bypass policy.

## Implementation steps

1. Add explicit exclusion rules and strict private-owner behavior with project/active-member checks first.
2. Resolve selected eligible member to Beta and every other eligible member to Published fallback.
3. Validate Beta target replacement as one transaction and emit effective per-member changes/tombstones.
4. Add audience preview with explanations for policy, Beta eligibility and effective version/channel.

## Required tests

- Full role/sub-role/tag/member/shared/private/include/exclude matrix.
- Ineligible, disabled, cross-project and excluded Beta targets reject the complete update.
- Non-selected member cannot fetch Beta through changes, metadata or artifact URL.
- Target removal/promotion produces fallback or tombstone idempotently.
- Preview and delivery use identical resolver fixtures.

## Definition of done and results

- [ ] Authorization suite covers browser roles and every client-token negative case.
- [ ] Resolver query is indexed and measured at projected membership volume.
- [ ] Exact outputs are recorded before Done.

## History

| Date | Status | Note |
|---|---|---|
| 2026-08-11 | Todo | DES-007 approved by project owner |
