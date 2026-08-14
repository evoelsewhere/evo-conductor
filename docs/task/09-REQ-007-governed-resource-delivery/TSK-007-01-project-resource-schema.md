# TSK-007-01 — Add the project-scoped resource schema and domain

| | |
|---|---|
| ID | TSK-007-01 |
| Created / updated | 2026-08-11 / 2026-08-14 |
| Status | Implemented — general REQ-001/REQ-003 and PostgreSQL proof remain |
| Layer | BE (Rust) |
| Requirement | [REQ-007](../../requirement/09-REQ-007-resource-lifecycle.md), coordinated REQ-008/012/013 |
| Design | [DES-007 sections 3–5](../../design/09-DES-007-governed-resource-delivery.md) |
| Covers | REQ-007 AC-1–AC-6, AC-31–AC-36; REQ-013 AC-2, AC-15–AC-17 |
| Depends on | REQ-001 and REQ-003 implementation |
| Estimate | 3d |
| Branch | `feat/REQ-007-governed-resource-delivery` |

## Goal

Create the database-enforced project/resource/version identity and strict Rust domain types on which all
later authoring, release, synchronization and inventory work depends.

## Files in scope

| File | Action |
|---|---|
| `crates/conductor-domain/src/resource.rs` | Replace legacy kind/status/request shapes with typed project, Plugin, channel and release models |
| `crates/conductor-storage/src/migrate.rs` or versioned migration files | Add project-scoped resource/version/channel/change/inventory schema |
| `crates/conductor-storage/src/repos/resource.rs` | Make every lookup/write project-scoped |
| storage/domain tests | Add fresh/upgrade and constraint coverage |

## Implementation steps

1. Add `Plugin`, release channel, observed state, version mode and safe error enums; migrate or reject the legacy technical kind explicitly.
2. Add composite project ownership, release-channel, Beta-member, change and inventory tables/indexes from DES-007 section 4.
3. Backfill existing rows into the only V1 project without inferring ownership from slug.
4. Require project context in every repository method and reject cross-project foreign keys.

## Required tests

- Domain serialization and invalid enum/state cases.
- Fresh and previous-schema migrations on SQLite and PostgreSQL.
- Duplicate slug allowed across projects but rejected within one project.
- Cross-project resource/version/channel/inventory references fail transactionally.
- Legacy kind fixture has deterministic migrate-or-error behavior.

## Definition of done and results

- [ ] `cargo fmt --check`, clippy, workspace tests and PostgreSQL migration proof pass.
- [ ] No repository resource write exists without project scope.
- [ ] Results contain exact command output and migration evidence; until then status cannot leave Blocked/Todo.

### Current evidence — 2026-08-14

`cargo test --workspace` passes 94 tests, including fresh/idempotent schema and governed-resource storage
tests. PostgreSQL and general migration-version tracking remain unverified, so this task is implemented
rather than closed as Done.

## History

| Date | Status | Note |
|---|---|---|
| 2026-08-11 | Todo | Design approved; waits only for prerequisite implementation |
| 2026-08-14 | Implemented | Project-scoped resource domain/schema landed; full versioned migrations and multi-project membership remain separate requirements |
