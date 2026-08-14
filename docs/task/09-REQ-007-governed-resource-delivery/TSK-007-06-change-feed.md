# TSK-007-06 — Expose cursor changes and authorized artifacts

| | |
|---|---|
| ID | TSK-007-06 |
| Created / updated | 2026-08-11 / 2026-08-14 |
| Status | Implemented on Conductor — EvoFlux smart-fetch migration remains in TSK-007-08 |
| Layer | BE (Rust) |
| Requirement | [REQ-012](../../requirement/13-REQ-012-resource-sync-client.md) |
| Design | [DES-007 section 6.2](../../design/09-DES-007-governed-resource-delivery.md) |
| Covers | REQ-012 AC-7–AC-15, AC-23, AC-29–AC-35, AC-54 |
| Depends on | TSK-007-03, TSK-007-04, TSK-007-05 |
| Estimate | 3d |
| Branch | `feat/REQ-007-governed-resource-delivery` |

## Goal

Provide the schema-v2, project/member-bound, replayable change feed and effective-version payload/artifact
endpoints required for offline-safe EvoFlux reconciliation.

## Implementation steps

1. Encode opaque cursors bound to schema, project, member and monotonic sequence; paginate deterministically.
2. Emit full stable identity, channel, digest, compatibility, trust and tombstone fields.
3. Re-run effective audience on metadata and artifact reads and stream immutable content.
4. Keep the old snapshot as a time-bounded Agent/Skill compatibility path with no Plugin artifact support.

## Required tests

- Pagination, replay, new changes during paging and invalid/stale/cross-member cursors.
- Beta add/remove/promotion/archive/unassignment/lost-access change sequences.
- Revoked/expired/wrong-scope/disabled/cross-project token matrix.
- Direct non-effective version and artifact reads return `403`.
- Query p95 target and response limits from DES-007 section 11.

## Definition of done and results

- [ ] Rust API contract tests, clippy/build and measured query proof pass.
- [ ] OpenAPI/example fixtures are consumable by EvoFlux Pydantic models.
- [ ] Exact output and compatibility evidence are recorded.

### Current evidence — 2026-08-14

The Rust run passes cursor/Bundle V2 tests and `smart_fetch_negotiates_delta_objects_and_tombstones`.
The 1,000-member reference run measured change-feed p95 at 190 ms. An OpenAPI artifact and EvoFlux
smart-fetch consumer are still missing.

## History

| Date | Status | Note |
|---|---|---|
| 2026-08-11 | Todo | DES-007 approved by project owner |
| 2026-08-14 | Implemented | HMAC cursor feed, descriptor/artifact authorization, ETag caching, realtime invalidation and smart fetch landed |
