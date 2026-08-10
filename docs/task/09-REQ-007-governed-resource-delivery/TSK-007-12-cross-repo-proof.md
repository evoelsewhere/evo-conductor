# TSK-007-12 — Prove cross-repository security and convergence

| | |
|---|---|
| ID | TSK-007-12 |
| Created / updated | 2026-08-11 |
| Status | Todo — waits for TSK-007-01 through TSK-007-11 |
| Layer | Infra/QA |
| Requirement | REQ-007, REQ-008, REQ-010, REQ-012 and REQ-013 |
| Design | [DES-007 sections 12–14](../../design/09-DES-007-governed-resource-delivery.md) |
| Covers | All coordinated acceptance criteria and V1 criterion 14 |
| Depends on | TSK-007-01 through TSK-007-11 |
| Estimate | 4d |
| Branch | `feat/REQ-007-governed-resource-delivery` in both repositories |

## Goal

Provide repeatable contract, security and end-to-end evidence that the complete epic converges without
Beta leaks, version duplication, cross-project adoption, user-file overwrite or automatic Plugin execution.

## Implementation steps

1. Add a versioned shared fixture corpus for valid/malicious Agent, Skill and Plugin packages.
2. Run two projects with identical slugs, two members with different policies and at least two installations.
3. Exercise import/edit/Auto and Manual release/Beta pull/trust/promotion/target removal/archive/inventory.
4. Inject network loss, retry, crash-before-cursor, concurrent publish, modified local content and revocation.
5. Add CI jobs for Rust SQLite/PostgreSQL, Conductor web, EvoFlux Python/web and cross-repo contract tests.

## Required evidence

- Exact request/resource/version/channel/digest and inventory convergence for both members/projects.
- One successful concurrent version allocation, no skipped number and deterministic conflict.
- Non-selected/cross-project direct artifact requests denied.
- Plugin remains disabled until local trust; decline/failure preserves old runtime/private data.
- No prompt, response, reasoning, file content/path, argument/result, environment value or credential on wire.
- Playwright desktop/mobile screenshots and HTML reports for both products.

## Definition of done and results

- [ ] All repository baseline commands and real release builds pass.
- [ ] Both real wire boundaries are exercised; mocks alone do not satisfy this task.
- [ ] Results contain unmodified commands, reports, fixture digests and screenshot links.
- [ ] Any deviation updates DES-007 before implementation is considered complete.

## History

| Date | Status | Note |
|---|---|---|
| 2026-08-11 | Todo | DES-007 approved; implementation dependencies remain |
