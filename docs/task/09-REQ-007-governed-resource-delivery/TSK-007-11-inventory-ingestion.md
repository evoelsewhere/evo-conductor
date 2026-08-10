# TSK-007-11 — Ingest desired-versus-observed resource inventory

| | |
|---|---|
| ID | TSK-007-11 |
| Created / updated | 2026-08-11 |
| Status | Todo |
| Layer | BE (Rust) |
| Requirement | [REQ-013](../../requirement/14-REQ-013-inventory-synchronization.md) |
| Design | [DES-007 sections 4, 6.2 and 10](../../design/09-DES-007-governed-resource-delivery.md) |
| Covers | REQ-013 AC-1–AC-17 |
| Depends on | TSK-007-05, TSK-007-08, TSK-007-09 |
| Estimate | 3d |
| Branch | `feat/REQ-007-governed-resource-delivery` |

## Goal

Accept a typed, idempotent, project-scoped installation/resource inventory and expose real online,
version/channel drift, trust, compatibility and sync-error data to authorized Conductor views.

## Implementation steps

1. Finalize the allowlisted inventory schema and EvoFlux collector contract without content/private values.
2. Transactionally upsert installation and observed resources keyed by project/installation/resource.
3. Reject the complete report on conflicting project/resource IDs; ignore client claims about Beta audience.
4. Join observed rows to the server's effective desired version/channel and expose paginated filters/detail.
5. Wire resource/member monitoring panels through typed FE APIs as part of TSK-007-07.

## Required tests

- Duplicate report updates rather than inserts; multi-installation member remains distinct.
- Cross-project row rejects all changes; prior inventory remains intact.
- `trust_pending` is not healthy/in-sync; desired/applied/channel drift is exact.
- Payload schema cannot carry package content, instructions, args, values, credentials or paths.
- User self-only, Admin cross-member and Contributor policy cases are explicit.

## Definition of done and results

- [ ] Rust checks plus EvoFlux contract fixture tests pass.
- [ ] `members_online` is proven from actual heartbeat/inventory data, not a fabricated default.
- [ ] Exact outputs and populated/empty/error UI evidence are recorded.

## History

| Date | Status | Note |
|---|---|---|
| 2026-08-11 | Todo | DES-007 approved by project owner |
