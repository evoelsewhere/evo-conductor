# TSK-007-02 — Build safe Draft import and validation

| | |
|---|---|
| ID | TSK-007-02 |
| Created / updated | 2026-08-11 / 2026-08-14 |
| Status | Implemented for UTF-8 bundles — shared cross-repo fixtures remain |
| Layer | BE (Rust) |
| Requirement | [REQ-007](../../requirement/09-REQ-007-resource-lifecycle.md), [REQ-010](../../requirement/19-REQ-010-plugin-distribution-safety.md) |
| Design | [DES-007 sections 4, 6 and 10](../../design/09-DES-007-governed-resource-delivery.md) |
| Covers | REQ-007 AC-7–AC-9, AC-19–AC-30; REQ-010 AC-1, AC-2, AC-8, AC-9 |
| Depends on | TSK-007-01 |
| Estimate | 4d |
| Branch | `feat/REQ-007-governed-resource-delivery` |

## Goal

Provide server-owned Draft workspaces, guides/templates, safe ZIP extraction, file APIs and static
Agent/Skill/Plugin validation without executing uploaded content.

## Files in scope

| Area | Action |
|---|---|
| Conductor domain/server | Add diagnostics, guide/template and Draft file request/response types/routes |
| New authoring service | Add path confinement, quarantine, extraction, limits and deterministic validation |
| `resource-authoring-guide.md` fixtures | Add valid and malicious shared packages |

## Implementation steps

1. Create Draft roots only from server-issued project/resource IDs and implement revisioned UTF-8 file CRUD.
2. Inspect archives before extraction; reject traversal, collisions, symlinks, unsupported entries and bombs.
3. Implement kind validators and masked secret diagnostics; validation never imports or runs package code.
4. Expose guides/templates/import/tree/file/validate/acknowledge endpoints with owner authorization.

## Required tests

- Malicious archive corpus plus wrapper normalization and repairable invalid packages.
- Agent frontmatter, Skill bundle and Plugin manifest/layout/name/size fixtures shared with EvoFlux.
- Absolute/backslash/traversal paths and symlink races rejected.
- Admin/owner Contributor/non-owner Contributor/User route matrix.
- Saving with stale Draft revision returns `409` without lost content.

## Definition of done and results

- [ ] Rust static/build/test commands pass; no package process is started by tests or implementation.
- [ ] Every documented starter passes both validator suites.
- [ ] Exact outputs and fixture digests are recorded before Done.

### Current evidence — 2026-08-14

The 94-test Rust run includes Agent, Skill and Plugin imports, invalid/repairable archives, traversal,
case-collision, revision conflict and static validator coverage. A shared fixture digest consumed by both
repositories remains missing.

## History

| Date | Status | Note |
|---|---|---|
| 2026-08-11 | Todo | DES-007 approved by project owner |
| 2026-08-14 | Implemented | Agent/Skill/Plugin import, Draft CRUD, archive safety and structured validators landed with focused Rust tests |
