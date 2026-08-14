# TSK-007-09 — Integrate Plugin staging, trust and atomic update

| | |
|---|---|
| ID | TSK-007-09 |
| Created / updated | 2026-08-11 / 2026-08-14 |
| Status | Implemented — packaged cross-repo trust E2E remains |
| Layer | EvoFlux (Python) |
| Requirement | [REQ-010](../../requirement/19-REQ-010-plugin-distribution-safety.md), [REQ-012](../../requirement/13-REQ-012-resource-sync-client.md) |
| Design | [DES-007 sections 5.3, 9 and 10](../../design/09-DES-007-governed-resource-delivery.md) |
| Covers | REQ-010 AC-1–AC-16; REQ-012 AC-15–AC-22, AC-27, AC-28, AC-33, AC-53 |
| Depends on | TSK-007-06, TSK-007-08 |
| Estimate | 4d |
| Branch | `feat/REQ-007-governed-resource-delivery` in `evoflux` |

## Goal

Route Conductor Plugin artifacts through EvoFlux's existing validator/installer/trust platform, preserving
installation identity and private data while requiring visible local approval for executable changes.

## Implementation steps

1. Download to staging with bounded stream, size/SHA-256 verification and schema/client compatibility check.
2. Map `(project_id, resource_id)` to stable Plugin installation ID and install first receipt disabled.
3. Build trust-surface diff from manifest, files, Skills, commands, hosts, environment field names and capabilities.
4. Preserve prior runnable version, credentials and `PLUGIN_DATA` on update failure/decline; disable on tombstone.
5. Report only typed pending/active/error metadata to inventory.

## Required tests

- First receipt never starts a process or enables contributed Skills before trust.
- Executable-surface change re-prompts; metadata-only identical digest does not.
- Failed/malicious/incompatible update leaves last-known-good runtime and private data intact.
- Same-name local Plugin is never adopted; cross-project artifact is rejected before install.
- Removal preserves credentials/data unless the member explicitly deletes locally.

## Definition of done and results

- [ ] EvoFlux Python checks and Plugin platform tests pass.
- [ ] Static trust test proves values and artifact paths never enter Conductor payloads/logs.
- [ ] Exact test output and state-transition evidence are recorded.

### Current evidence — 2026-08-14

Focused EvoFlux governed-reconciler tests cover first-install `trust_pending`, replay, update review,
stable installation mapping, validation failures and preservation of last-known-good state; the current
four-file focused suite passes 37 tests. A packaged real-Conductor trust flow remains.

## History

| Date | Status | Note |
|---|---|---|
| 2026-08-11 | Todo | DES-007 approved by project owner |
| 2026-08-14 | Implemented | Plugin staging, digest validation, stable mapping, trust/update pending and prior-runtime preservation landed |
