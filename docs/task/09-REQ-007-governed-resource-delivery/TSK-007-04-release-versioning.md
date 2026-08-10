# TSK-007-04 — Implement transactional releases and semantic versioning

| | |
|---|---|
| ID | TSK-007-04 |
| Created / updated | 2026-08-11 |
| Status | Todo |
| Layer | BE (Rust) |
| Requirement | [REQ-007](../../requirement/09-REQ-007-resource-lifecycle.md) |
| Design | [DES-007 sections 5.1 and 6](../../design/09-DES-007-governed-resource-delivery.md) |
| Covers | REQ-007 AC-1–AC-5, AC-10–AC-13, AC-33–AC-38 |
| Depends on | TSK-007-01, TSK-007-02, TSK-007-03 |
| Estimate | 3d |
| Branch | `feat/REQ-007-governed-resource-delivery` |

## Goal

Make Beta/direct Publish allocate immutable versions on the server, with correct Auto/Manual SemVer,
Plugin manifest synchronization, promotion, rollback-as-new-version and audit behavior.

## Implementation steps

1. Add a strict SemVer 2.0 parser/comparator and next-version service; default first release to `0.1.0`.
2. Implement one transaction for Draft revision check, version allocation, packaging, immutable row, channel binding and audit.
3. Preserve version ID/bytes on Beta promotion; allocate a new greater version when restoring old content.
4. Return safe `422` field diagnostics and `409 version_conflict` with refreshed highest/next values.

## Required tests

- SemVer table: stable, prerelease, build metadata, major/minor manual bumps and every invalid form.
- Save/validate/target/deprecate/archive do not increment; promotion does not increment.
- Failed validation/auth/storage consumes no version.
- Concurrent releases yield one allocation and one deterministic conflict.
- Auto and Manual Plugin manifest values exactly match artifact/version metadata.

## Definition of done and results

- [ ] Rust commands and concurrent PostgreSQL proof pass.
- [ ] Audit rows record mode, prior highest, request, allocation and version ID without payloads.
- [ ] Exact test output is attached before Done.

## History

| Date | Status | Note |
|---|---|---|
| 2026-08-11 | Todo | DES-007 approved by project owner |
