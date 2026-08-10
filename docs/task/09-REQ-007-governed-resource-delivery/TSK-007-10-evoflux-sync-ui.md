# TSK-007-10 — Build EvoFlux sync, diff and trust-review UI

| | |
|---|---|
| ID | TSK-007-10 |
| Created / updated | 2026-08-11 |
| Status | Todo |
| Layer | EvoFlux FE (React) |
| Requirement | [REQ-012](../../requirement/13-REQ-012-resource-sync-client.md) |
| Design | [DES-007 section 9](../../design/09-DES-007-governed-resource-delivery.md) |
| Covers | REQ-012 AC-12, AC-14, AC-16–AC-18, AC-22, AC-27, AC-32, AC-36 |
| Depends on | TSK-007-08, TSK-007-09 |
| Estimate | 3d |
| Branch | `feat/REQ-007-governed-resource-delivery` in `evoflux` |

## Goal

Show honest project-scoped synchronization, diff, compatibility, ownership conflict and Plugin trust
states, with safe member actions that do not imply server delivery equals local activation.

## Implementation steps

1. Extend the existing Conductor settings/status UI and typed API/query modules.
2. Add project identity, resource/version/channel, last run and pending/error summary.
3. Add Agent/Skill canonical diff and Plugin trust-surface review with approve/decline/defer actions.
4. Add project-switch warning and old-project disable progress without exposing local absolute paths.

## Required tests

- Component coverage for staged, trust/update pending, applied, declined, incompatible, conflict, offline and project mismatch.
- Role-independent local trust actions remain local and never modify Conductor audience.
- Back/forward and restart preserve pending review state.
- Playwright desktop/mobile screenshots for connect, diff, approve, decline and error recovery.
- Accessibility checks for focus, keyboard, non-color state and long file/capability lists.

## Definition of done and results

- [ ] EvoFlux web lint/typecheck/unit/e2e/build scripts pass.
- [ ] Screenshots and exact reports are attached before Done.
- [ ] No credential, command argument, environment value or absolute path renders.

## History

| Date | Status | Note |
|---|---|---|
| 2026-08-11 | Todo | DES-007 approved by project owner |
