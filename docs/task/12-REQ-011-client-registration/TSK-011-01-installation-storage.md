# TSK-011-01 — Add installation registration storage

| | |
|---|---|
| ID | TSK-011-01 |
| Created | 2026-08-10 |
| Updated | 2026-08-10 |
| Status | Draft planning — do not start until DES-011 is approved |
| Layer | Conductor BE |
| Requirement | [REQ-011](../../requirement/12-REQ-011-client-registration.md) |
| Design | [DES-011 sections 4 and 6](../../design/12-DES-011-client-registration.md#4-data-model-changes) |
| Covers | AC-2, AC-3, AC-8, AC-11 |
| Depends on | REQ-001, REQ-006 and REQ-015 accepted |
| Estimate | 1.5d |
| Branch | `feat/REQ-011-client-registration` |

## 1. Goal

Create portable, authoritative persistence for EvoFlux installations. A registration must atomically return
one server-issued ID for one `(instance_id, installation_key)`; a heartbeat can refresh only the matching
member's installation.

## 2. Files in scope

| File | Action |
|---|---|
| `crates/conductor-domain/src/client_installation.rs` | Create validated entity, platform and persistence DTOs. |
| `crates/conductor-domain/src/lib.rs` | Modify exports. |
| `crates/conductor-storage/src/migrate.rs` | Add table/index/idempotency migration. |
| `crates/conductor-storage/src/repos/client_installation.rs` | Create transactional upsert, replay and heartbeat repository. |
| `crates/conductor-storage/src/repos/mod.rs` | Modify exports. |
| `crates/conductor-storage/tests/client_installation_repository.rs` | Create SQLite repository/migration tests. |

## 3. Implementation steps

1. Define bounded input validation: reject blank labels, unsupported platforms, invalid UUIDs and
   path-like workspace associations before the database boundary.
2. Add `client_installations` and narrowly scoped idempotency records. Store request hash/status/response
   only for the replay window; never store raw token values.
3. Add one transactional operation that resolves instance, upserts by local key, updates timestamps, and
   returns the canonical server installation ID.
4. Add a heartbeat update predicate containing installation ID, instance ID and owner user ID. Return
   `None` on mismatch without exposing another member's record.
5. Test migrations against both an empty SQLite database and the current schema.

## 4. Required tests

| Type | Tool | Must cover |
|---|---|---|
| Unit, pure domain | `cargo test` in `conductor-domain` | Validation limits, platform values, UUID parsing and path rejection. |
| Repository | `cargo test` with sqlx `sqlite::memory:` | First upsert, same-key update, two keys/member, unique constraint, scoped heartbeat and migration. |
| Transaction regression | `cargo test` | Same idempotency key/same request replay; same key/different body conflict; no partial write on failure. |

## 5. Commands and reports

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test -p conductor-domain -p conductor-storage
```

## 6. Definition of done

- [ ] Additive schema works on SQLite and PostgreSQL-compatible `Any` SQL.
- [ ] Local reconciliation key never becomes the public server installation ID.
- [ ] No raw `evc_` value can be stored by repository or migration.
- [ ] Covered ACs have automated passing tests recorded in section 7.

## 7. Results

### Traceability: acceptance criteria to tests

| AC | Test case | File | Result |
|---|---|---|---|
| AC-2 | Not run — planning task | — | Pending |
| AC-3 | Not run — planning task | — | Pending |
| AC-8 | Not run — planning task | — | Pending |
| AC-11 | Not run — planning task | — | Pending |

### Command output

```text
Not run — planning task.
```

## History

| Date | Status | Note |
|---|---|---|
| 2026-08-10 | Draft planning | Created before design approval at user request |
