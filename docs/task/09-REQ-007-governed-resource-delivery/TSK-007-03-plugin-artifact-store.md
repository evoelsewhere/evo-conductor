# TSK-007-03 — Add immutable Plugin artifact storage

| | |
|---|---|
| ID | TSK-007-03 |
| Created / updated | 2026-08-11 / 2026-08-14 |
| Status | Implemented and expanded — buffered-response/production proof remains |
| Layer | BE (Rust) |
| Requirement | [REQ-007](../../requirement/09-REQ-007-resource-lifecycle.md), [REQ-010](../../requirement/19-REQ-010-plugin-distribution-safety.md) |
| Design | [DES-007 sections 2, 4 and 6](../../design/09-DES-007-governed-resource-delivery.md) |
| Covers | REQ-007 AC-14–AC-18, AC-25, AC-37; REQ-010 AC-2, AC-4, AC-6, AC-7 |
| Depends on | TSK-007-01, TSK-007-02 |
| Estimate | 2d |
| Branch | `feat/REQ-007-governed-resource-delivery` |

## Goal

Introduce a content-addressed `ArtifactStore` with a filesystem V1 backend so immutable Plugin packages
can be streamed, verified and referenced without placing credentials or mutable installation data inside them.

## Implementation steps

1. Define async put/open/delete-if-unreferenced operations and configure a server data root.
2. Deterministically pack validated Plugin Drafts into staging, stream SHA-256/length, fsync and atomic rename.
3. Store artifact key, schema, package identity, compatibility, digest and length on immutable version rows.
4. Stream authorized downloads with safe headers; never expose filesystem paths or public bypass URLs.

## Required tests

- Deterministic bytes/digest across repeated packs and mismatch rejection.
- Interrupted/stale writes leave no published metadata and are safely swept.
- Large artifact is streamed rather than buffered; length/digest headers match.
- Credential/environment marker corpus is blocked from release.
- Artifact lookup still requires effective-version authorization.

## Definition of done and results

- [ ] Rust fmt/clippy/tests/build pass on SQLite and PostgreSQL metadata paths.
- [ ] Filesystem backend limitation for multi-replica deployment is documented.
- [ ] Command output and measured streaming memory evidence are recorded.

### Current evidence — 2026-08-14

The 94-test Rust run covers deterministic ZIPs, Local object migration, Git push/credential rollback and
admin migration between backends. `docs/object-storage.md` documents Local/S3/Azure/Git. The artifact
HTTP response is still buffered and PostgreSQL/streaming memory evidence is absent.

## History

| Date | Status | Note |
|---|---|---|
| 2026-08-11 | Todo | DES-007 approved by project owner |
| 2026-08-14 | Implemented | Content-addressed Local/S3/Azure/Git storage, Bundle V2 and verified migration landed; artifact responses still buffer bytes |
