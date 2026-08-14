# TSK-007-07 — Build Resource Studio and release UI

| | |
|---|---|
| ID | TSK-007-07 |
| Created / updated | 2026-08-11 / 2026-08-14 |
| Status | Implemented — repository-wide frontend unit/e2e tooling remains absent |
| Layer | FE (React) |
| Requirement | [REQ-007](../../requirement/09-REQ-007-resource-lifecycle.md), [REQ-008](../../requirement/10-REQ-008-resource-access-policy.md) |
| Design | [DES-007 section 8](../../design/09-DES-007-governed-resource-delivery.md) |
| Covers | REQ-007 AC-19–AC-29, AC-38; REQ-008 AC-8, AC-9, AC-11, AC-13 |
| Depends on | TSK-007-02, TSK-007-04, TSK-007-05 |
| Estimate | 5d |
| Branch | `feat/REQ-007-governed-resource-delivery` |

## Goal

Replace the JSON textarea workflow with reusable Resource Studio screens for guides/templates, upload,
Monaco editing, diagnostics, Auto/Manual release, Beta members, audience preview and version history.

## Implementation steps

1. Add typed API/query modules and domain constants for kinds, channels, states, modes and error codes.
2. Build routed editor with responsive file tree, lazy Monaco, save/dirty/navigation guards and diagnostics.
3. Build release dialog with highest/next version, Manual validation, manifest diff, changelog and Beta selector.
4. Add audience/version/inventory panels and accessible loading/empty/error/forbidden/conflict states.

## Required tests

- Unit tests for SemVer field presentation and API error mapping; server remains authoritative.
- Component tests for every editor/release/audience state and role-based action visibility.
- Keyboard, focus, screen-reader diagnostic and mobile layout checks.
- Playwright create/import/edit/validate/Beta/Publish flow with desktop/mobile screenshots.
- Chart/inventory panels do not hard-code server enum literals.

## Definition of done and results

- [ ] Declared typecheck/lint/unit/e2e/build scripts pass.
- [ ] Screenshots and real report paths are embedded in Results.
- [ ] No existing EvoFlux visual-language or shared-component duplication is introduced.

### Current evidence — 2026-08-14

`bun run typecheck` and `bun run build` pass; the build retains only Vite compatibility/chunk-size
warnings. This repository still has no lint, Vitest or committed Playwright scripts, so UI behavior is
implemented without satisfying the complete test definition of done.

## History

| Date | Status | Note |
|---|---|---|
| 2026-08-11 | Todo | DES-007 approved by project owner |
| 2026-08-14 | Implemented | Create/import drawers, Monaco workbench, validation, releases, version history, access and monitoring UI landed |
