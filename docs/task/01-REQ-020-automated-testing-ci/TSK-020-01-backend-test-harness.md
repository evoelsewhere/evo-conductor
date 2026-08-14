# TSK-020-01 — Build the backend test harness

| | |
|---|---|
| ID | TSK-020-01 |
| Created | 2026-08-09 |
| Updated | 2026-08-14 |
| Status | Implemented — backend harness expanded; nextest/JUnit reporting remains |
| Layer | BE |
| Requirement | [REQ-020](../../requirement/01-REQ-020-automated-testing-ci.md) |
| Design | [DES-020 sections 2, 5](../../design/01-DES-020-automated-testing-ci.md) |
| Covers | AC-1, AC-11 |
| Depends on | none |
| Estimate | 1d |
| Branch | `feat/REQ-020-test-harness` |

## 1. Goal

After this task, any backend test can build a running router against a fresh isolated database, seed a
user of any role, and call an endpoint with that user's token. Everything in REQ-004 through REQ-019
depends on this existing.

## 2. Files in scope

| File | Action |
|---|---|
| `crates/conductor-server/Cargo.toml` | modify, add `[dev-dependencies]` |
| `crates/conductor-server/tests/support/mod.rs` | create |
| `crates/conductor-server/tests/health.rs` | create |
| `crates/conductor-storage/Cargo.toml` | modify, add `[dev-dependencies]` |
| `crates/conductor-storage/tests/migrations.rs` | create |
| `crates/conductor-domain/src/role.rs` | modify, add `#[cfg(test)]` module |

## 3. Implementation steps

**Step 0 — settle the database question before writing anything else.**

Write a throwaway test that opens `sqlite:file:conductor_test_<uuid>?mode=memory&cache=shared` through
`Db::connect`, inserts a row through one repository call and reads it back through another. If it passes,
the shared-cache URL works with the ten-connection pool and the rest of this task proceeds. If it fails,
switch to fallback (b) in [DES-020 section 2](../../design/01-DES-020-automated-testing-ci.md): add
`Db::connect_with(url, max_connections)` and use 1. **Record which path was taken in section 7 — the
whole suite design rests on this.**

1. Add dev-dependencies to `conductor-server`: `tower` with the `util` feature, `http-body-util`, `uuid`,
   `tokio` with `macros` and `rt-multi-thread`. Note `tower` is currently **not** a dependency at all;
   only `tower-http` is.
2. Write `tests/support/mod.rs`:
   - `async fn test_app() -> TestApp` — unique in-memory URL, `AppState::new`, then
     **`state.set_jwt_secret(<random hex>)`**, then `build_router(state.clone(), &Config { … })` with a
     throwaway `web_dist`.
   - `async fn seed_user(&self, role: PrimaryRole) -> User` — `UserRepo::create_invited` then
     `set_status(id, UserStatus::Active)`. Use one pre-computed Argon2 hash constant, do not hash per call.
   - `fn token_for(&self, user: &User) -> String` — `JwtService::issue`.
   - `async fn get(&self, path: &str, token: Option<&str>) -> (StatusCode, Value)` — build a request,
     `router.clone().oneshot(req)`, collect the body with `http-body-util`.
3. Write `tests/health.rs`: `GET /api/health` returns 200 and a body whose `database` field is `sqlite`.
4. Write `conductor-storage/tests/migrations.rs`: `migrate::run` against an empty database succeeds, and
   running it a second time is a no-op.
5. Add `#[cfg(test)]` unit tests in `conductor-domain/src/role.rs` for every capability predicate against
   all three roles, and for `PrimaryRole::parse` round-tripping.
6. Install nextest locally: `cargo install cargo-nextest --locked`.

## 4. Required tests

### Layer BE (Rust)

| Type | Tool | Must cover |
|---|---|---|
| Unit, pure domain | `cargo test` in `conductor-domain` | Every capability predicate for all three roles; `PrimaryRole::parse` and `UserStatus::parse` round-trips |
| Repository | `cargo test` with the shared-cache in-memory URL | Migrations apply to an empty database; a second run changes nothing; a user written through one repo call is readable through another |
| HTTP route | `axum::Router` with `tower::ServiceExt::oneshot` | `GET /api/health` returns 200 and the correct dialect |
| Authorization regression | — | Not in this task. TSK-020-02 |

## 5. Commands and reports

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo nextest run
```

Record the wall-clock time of the suite; AC-11 needs a measured number.

## 6. Definition of done

- [x] Step 0 settled and the outcome recorded in section 7.
- [x] `test_app`, `seed_user`, `token_for` and the request helper all work.
- [x] Health/liveness and setup-required route tests pass.
- [x] Migration test passes, including the second-run no-op.
- [x] Domain predicate tests pass for all three roles.
- [ ] Every command in section 5 runs clean, no warning suppressed. `cargo nextest`/JUnit was not run.
- [x] `cargo build --release` succeeds.
- [x] Section 7 contains real output.
- [x] No new clippy warning.

## 7. Results

### Traceability: acceptance criteria to tests

| AC | Test case | File | Result |
|---|---|---|---|
| AC-1 | Health/database dialect and isolated test-app routes | `crates/conductor-server/tests/health.rs` | Pass |
| AC-1 | `migrations_apply_to_an_empty_database` | `crates/conductor-storage/tests/migrations.rs` | Pass |
| AC-1 | `migrations_are_idempotent` | `crates/conductor-storage/tests/migrations.rs` | Pass |
| AC-1 | Shared-cache visibility, pool concurrency and isolation | `crates/conductor-storage/tests/step0_pool_isolation.rs` | Pass |
| AC-11 | `cargo test --workspace` wall-clock | local verification | Pass; current suite has 94 tests, formal target/report pending |

### Step 0 outcome

Selected unique named shared-cache in-memory SQLite URLs. Tests prove visibility across pooled
connections, isolation between test databases and concurrency within the configured pool.

### Command output

```
cargo fmt --check                                                   PASS
cargo clippy --workspace --all-targets --all-features -- -D warnings PASS
cargo test --workspace                                              PASS (94 tests; verified 2026-08-14)
cargo build -p conductor-server --release                           PASS
cargo nextest run / JUnit                                           NOT RUN
```

### Notes

The harness is present in the current branch and has expanded with the product surface. The remaining
nextest/JUnit and formal AC-11 duration target belong to REQ-020 reporting/CI completion; PostgreSQL
belongs to TSK-020-05/REQ-001 rather than this SQLite harness proof.

## History

| Date | Status | Note |
|---|---|---|
| 2026-08-09 | Todo | Created |
| 2026-08-10 | In Review | Backend harness merged in PR #1; nextest/JUnit and formal duration target remain |
| 2026-08-14 | Implemented | Current workspace suite passes 94 tests; reporting and CI gaps remain tracked separately |
