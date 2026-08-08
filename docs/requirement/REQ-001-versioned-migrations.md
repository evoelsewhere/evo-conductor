# REQ-001 — Versioned database migrations

| | |
|---|---|
| ID | REQ-001 |
| Created | 2026-08-09 |
| Updated | 2026-08-09 |
| Status | Draft |
| Priority | P0 |
| Spec section | [requirements.md section 15](../requirements.md) |
| Source | Baseline specification section 15, plus code review 2026-08-09 |
| Depends on | none |
| Blocks | REQ-003, REQ-007, REQ-009, REQ-013, REQ-014, REQ-016, REQ-018 |
| Repositories | `evo-conductor` |
| Design | Not created; requires acceptance |

## 1. Context

Section 15 of the specification requires seven new tables: `client_installations`, `client_heartbeats`,
`resource_versions`, `resource_access_policies`, `resource_sync_state`, `documents`, `usage_aggregates`
and `audit_events`. The current mechanism cannot report which migrations have been applied and discards
errors, so every schema change built on top of it is unverifiable.

The specification states the requirement directly: database changes should use versioned migrations
rather than only runtime `CREATE TABLE` and best-effort `ALTER TABLE` statements.

## 2. Requirement

Conductor shall apply schema changes through numbered, ordered migrations, shall record which migrations
have been applied, and shall fail loudly when a migration cannot be applied. The same migration sequence
shall apply cleanly to PostgreSQL and to SQLite.

## 3. Implementation status

| Implemented | Missing | Incorrect |
|---|---|---|
| Portable schema using TEXT identifiers and INTEGER flags, working across SQLite, PostgreSQL and MySQL ([migrate.rs:3](../../crates/conductor-storage/src/migrate.rs)) | A schema-version tracking table | `ALTER TABLE` errors are discarded with `let _ = ...` ([migrate.rs:166](../../crates/conductor-storage/src/migrate.rs)) |
| Idempotent `CREATE TABLE IF NOT EXISTS` statements | Any rollback path | A failed migration is indistinguishable from a successful one |
| A one-off backfill from `user_tags` into `tag_assignments` ([migrate.rs:170-178](../../crates/conductor-storage/src/migrate.rs)) | Migration ordering guarantees beyond array order | Startup continues after a partial migration |

## 4. Acceptance criteria

| ID | Criterion |
|---|---|
| AC-1 | A migration tracking table records applied migrations; running twice against an up-to-date database applies nothing |
| AC-2 | A failing migration aborts startup with an error naming the migration; the process does not continue serving requests |
| AC-3 | The entire current schema is expressed as numbered migration files that apply cleanly to an empty database |
| AC-4 | A database created by the previous mechanism upgrades to the new baseline without data loss |
| AC-5 | CI applies the full migration sequence against both PostgreSQL and SQLite |
| AC-6 | Each migration has a documented manual reversal procedure |
| AC-7 | `make reset-db` recreates a development database from the migration sequence alone |

## 5. Out of scope

- Adding any of the new tables; each belongs to the requirement that needs it.
- Automated production rollback tooling. A documented manual procedure satisfies AC-6.
- Removing MySQL support, which currently works and costs nothing to keep.

## 6. Risks

| # | Risk | Severity | Mitigation |
|---|---|---|---|
| 1 | Existing development databases do not match the new baseline | Medium | AC-4 plus `make reset-db`, which already exists |
| 2 | Dialect differences surface once statements are separated per migration | Medium | AC-5 runs both dialects in CI |
| 3 | The change looks like pure refactoring and gets deprioritized | High | It blocks seven other requirements; sequence it first |

## 7. Open questions

- Does any database currently hold data that must be preserved? If not, re-baselining from an empty
  schema is materially simpler than writing an upgrade path.
- Is PostgreSQL required for the V1 acceptance run, or only for production rollout? This determines
  whether AC-5 blocks V1.

## History

| Date | Change | Author |
|---|---|---|
| 2026-08-09 | Created | |
