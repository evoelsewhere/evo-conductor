# TSK-007-08 — Persist managed state and reconcile Agent and Skill resources

| | |
|---|---|
| ID | TSK-007-08 |
| Created / updated | 2026-08-11 / 2026-08-14 |
| Status | Implemented with cursor delivery — smart-fetch generation checkout remains |
| Layer | EvoFlux (Python) |
| Requirement | [REQ-012](../../requirement/13-REQ-012-resource-sync-client.md) |
| Design | [DES-007 sections 5.3 and 9](../../design/09-DES-007-governed-resource-delivery.md) |
| Covers | REQ-012 AC-1–AC-14, AC-20, AC-23–AC-30, AC-35–AC-48, AC-54 |
| Depends on | TSK-007-06 |
| Estimate | 5d |
| Branch | `feat/REQ-007-governed-resource-delivery` in `evoflux` |

## Goal

Replace kind/slug snapshot reconciliation with durable `(project_id, resource_id)` state, atomic
Agent/Skill updates, conflict-safe removals, cursor commit and isolated project switching.

## Files in scope

`app/conductor/models.py`, `client.py`, `reconciler.py`, `service.py`, a versioned local migration/model,
existing Agent/Skill loaders and focused tests.

## Implementation steps

1. Add schema-v2 models/client pagination and durable state/cursor migration.
2. Stage complete content, verify digest, compare actual/last-applied digest and atomically switch owned targets.
3. Keep user-owned/same-name/modified targets intact and expose canonical text/file diff metadata.
4. Commit cursor only after a durable result; replay interrupted pages idempotently.
5. Register project-scoped managed roots and implement safe disable-before-activate project switching.

## Required tests

- Decision matrix for same version/channel/digest, metadata-only change, changed content and rollback.
- Crash at each stage proves no skipped cursor or partial target.
- Same slugs in two projects remain isolated through load/update/remove.
- `AGENTS.override.md` and repository-tree writes never occur.
- Offline startup uses last-known-good content; revocation stops next cycle.

## Definition of done and results

- [ ] `ruff`, format check, `ty` and focused/full pytest pass.
- [ ] No new Agent/Skill runtime or raw secret/path reporting is introduced.
- [ ] Exact outputs and temporary-directory tree evidence are recorded.

### Current evidence — 2026-08-14

The current EvoFlux feature branch contains project-scoped managed state, cursor replay, digest/conflict,
tombstone, project mismatch and runtime provenance tests. Focused registration/reconcile/runtime/telemetry
verification passes 37 tests. It still consumes cursor pages rather than the new Conductor smart-fetch
checkout.

## History

| Date | Status | Note |
|---|---|---|
| 2026-08-11 | Todo | DES-007 approved by project owner |
| 2026-08-14 | Implemented | Project-scoped managed store, ownership/conflict handling, durable cursor and runtime provenance landed; smart fetch remains |
