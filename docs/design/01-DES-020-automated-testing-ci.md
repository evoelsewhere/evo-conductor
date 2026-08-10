# DES-020 — Automated testing and CI

| | |
|---|---|
| ID | DES-020 |
| Created | 2026-08-09 |
| Updated | 2026-08-10 |
| Status | Draft — partial implementation reconciliation |
| Requirement | [REQ-020](../requirement/01-REQ-020-automated-testing-ci.md) |
| Build order | Step 1 of 23 |
| References | [BASE-CONVENTIONS](../base/BASE-CONVENTIONS.md), [architecture.md](../architecture.md) |
| Tasks | TSK-020-01 … TSK-020-05, see section 13 |

## 1. Goal

Build the test infrastructure that every later task's definition of done depends on, and prove it works
by using it on a defect that already exists.

Acceptance criteria in scope: AC-1 through AC-11 of
[REQ-020](../requirement/01-REQ-020-automated-testing-ci.md).

## 2. Options considered

| Decision | Options | Chosen |
|---|---|---|
| Test runner | `cargo test`; `cargo nextest` | **nextest** |
| Test database | plain `sqlite::memory:`; named shared-cache memory; temp file | **named shared-cache memory** |
| HTTP invocation | bind a real port; `tower::ServiceExt::oneshot` | **oneshot** |
| Authentication in tests | call `POST /api/auth/login`; mint a JWT directly | **mint directly** |
| Test location | in-crate `#[cfg(test)]`; `tests/` integration | **both, split by layer** |
| Known-failing tests | omit; assert current wrong behaviour; `#[ignore]` with reason | **`#[ignore]` with reason** |

**Rationale for nextest.** AC-2 requires a JUnit XML report, which `cargo test` cannot produce. nextest
also runs each test in its own process, which matters here because `Db::connect` calls
`sqlx::any::install_default_drivers()` process-wide ([db.rs:35](../../crates/conductor-storage/src/db.rs))
and every test constructs its own `Db`.

**Rationale for the database choice — this is the central risk in this design.**

`Db::connect` hardcodes `max_connections(10)`
([db.rs:37-40](../../crates/conductor-storage/src/db.rs)). With a plain `sqlite::memory:` URL each pooled
connection opens its **own private empty database**. `migrate::run` executes on whichever connection the
pool hands out first; a later query may be served by a different connection that has no tables. The suite
would fail non-deterministically, and the failure would look like a bug in the code under test.

Two ways out:

- **(a) Chosen.** A per-test unique shared-cache URL:
  `sqlite:file:conductor_test_<uuid>?mode=memory&cache=shared`. All pooled connections attach to the same
  in-memory database, which is dropped when the pool closes. **No production code changes.**
- **(b) Fallback.** Add `Db::connect_with(url, max_connections)` and pass 1 in tests. Requires touching
  production code for a test-only concern.

Option (a) is chosen because it leaves production untouched, and because a pool of one would hide
concurrency behaviour that production has.

**This must be proven before anything is built on it.** Whether the `sqlx` `Any` driver forwards that URL
unchanged to the SQLite driver has never been exercised in this repository. TSK-020-01 verifies it first
and falls back to (b) if it does not hold.

Related: `ensure_sqlite_parent_dir` will derive an empty parent from that URL and call
`create_dir_all("")`, whose error is discarded by `let _ =`
([db.rs:95](../../crates/conductor-storage/src/db.rs)). This is harmless. Do not "fix" it.

**Rationale for minting JWTs directly.** `JwtService::issue(user_id, email, role)` is public
([jwt.rs:30](../../crates/conductor-auth/src/jwt.rs)). Going through the login endpoint would couple every
authorization test to the login flow and to Argon2 hashing, which is deliberately slow.

**Rationale for `#[ignore]` on known failures.** The authorization suite will fail on two endpoints that
REQ-004 and REQ-006 fix. Landing it red would block every merge; omitting those cases would lose the
evidence. Marking them `#[ignore = "expected failure until REQ-004"]` keeps the case in the repository,
keeps CI green, and turns the ignore list into a visible to-do. A non-blocking CI step runs
`--run-ignored all` and reports.

## 3. Data model changes

None. This design adds no table, column or migration.

## 4. API changes

None.

## 5. Backend changes

| Crate | File | Change |
|---|---|---|
| `conductor-server` | `Cargo.toml` | Add `[dev-dependencies]`: `tower` (for `ServiceExt::oneshot`), `http-body-util` (read response bodies under axum 0.8), `uuid`, `tokio` with `macros` and `rt-multi-thread` |
| `conductor-server` | `tests/support/mod.rs` | New. The fixture described below |
| `conductor-server` | `tests/health.rs` | New. Smoke test proving the fixture works |
| `conductor-server` | `tests/authorization.rs` | New. The regression matrix, AC-8 |
| `conductor-storage` | `tests/migrations.rs` | New. Migrations apply cleanly to an empty database, AC-1 |
| `conductor-domain` | `src/role.rs`, `src/user.rs` | Add `#[cfg(test)]` unit tests for the capability predicates and `UserStatus::parse` round-trips |

Everything the fixture needs is already public: `conductor_server::{build_router, AppState, Config}`
([lib.rs:3-7](../../crates/conductor-server/src/lib.rs)), `Config`'s fields are public
([config.rs](../../crates/conductor-server/src/core/config.rs)), and `AppState::set_jwt_secret` is public
([state.rs](../../crates/conductor-server/src/core/state.rs)). No production signature changes.

### The fixture

```
TestApp {
    router: Router,       // build_router(state, &config)
    state:  AppState,
    jwt:    JwtService,
}

async fn test_app() -> TestApp
async fn seed_user(&self, role: PrimaryRole) -> User
fn      token_for(&self, user: &User) -> String
async fn get(&self, path, token: Option<&str>) -> (StatusCode, serde_json::Value)
```

Three facts drive its construction:

1. `AppState::new(url)` connects, runs migrations, and reads `jwt_secret` from the `instance` row
   ([state.rs](../../crates/conductor-server/src/core/state.rs)). A fresh test database has no instance
   row, so `jwt` stays `None` and `AuthUser` returns **428 `SetupRequired`**, not 401
   ([auth_user.rs:28](../../crates/conductor-server/src/http/extractors/auth_user.rs)). The fixture must
   call `state.set_jwt_secret(...)` explicitly. A test that forgets this fails with a confusing 428.
2. `AuthUser` admits only `UserStatus::Active` ([auth_user.rs:40-44](../../crates/conductor-server/src/http/extractors/auth_user.rs)).
   Seeding uses `UserRepo::create_invited(&CreateMemberRequest, password_hash, invited_by)`
   ([user.rs:252](../../crates/conductor-storage/src/repos/user.rs)) followed by
   `set_status(id, Active)` ([user.rs:408](../../crates/conductor-storage/src/repos/user.rs)).
3. `build_router` mounts a `ServeDir` fallback from `config.web_dist`
   ([http/mod.rs:22-25](../../crates/conductor-server/src/http/mod.rs)). Tests point it at a throwaway
   path; API routes are unaffected.

## 6. Frontend changes

| File | Change |
|---|---|
| `apps/web/package.json` | Add devDeps `vitest`, `@vitest/coverage-v8`, `@testing-library/react`, `@testing-library/user-event`, `jsdom`, `@playwright/test`, `eslint` with the TypeScript and React plugins. Add scripts `test:unit`, `test:e2e`, `lint` |
| `apps/web/vitest.config.ts` | New. jsdom environment, reuse the `@` alias from `vite.config.ts` |
| `apps/web/eslint.config.js` | New. Flat config |
| `apps/web/playwright.config.ts` | New. HTML reporter, screenshot and trace on failure |
| `apps/web/src/shared/api/client.test.ts` | New. `request<T>` attaches the bearer token, prefixes `/api`, throws on non-2xx |
| `apps/web/src/features/resources/pages/resources-page.test.tsx` | New. All four render states |

The resources page is chosen for the first component test because it already exhibits every state and its
empty state currently makes a claim the backend cannot honour, which the test documents.

## 7. EvoFlux changes

Not applicable.

## 8. Security and authorization

- Test fixtures generate a random JWT secret per run. No secret is committed.
- The authorization matrix is the security-relevant deliverable: every endpoint against `admin`,
  `contribute` and `user`, asserting `200` or `403` explicitly rather than "not 500".
- CI must not print environment variables or database URLs on failure.

## 9. Performance

Target: the backend suite completes in under 60 seconds locally (AC-11). Each test builds its own
in-memory database, which is cheap; Argon2 hashing is the expensive part, so seeding uses a fixed
pre-computed hash rather than hashing per test.

## 10. Rollout and rollback

Purely additive. No runtime behaviour changes. Rollback is deleting the test directories and the workflow
file.

Sequencing constraint: TSK-020-05 (CI) must land **after** TSK-020-01, otherwise CI has nothing to run and
the first green build is meaningless.

## 11. Test strategy

This design builds the test infrastructure, so the infrastructure itself is proven by use rather than by
meta-tests:

- The fixture is proven by `tests/health.rs` returning 200 with a body naming the dialect.
- The database choice is proven by a test that seeds a user through one repository call and reads it back
  through another, which fails immediately if the pool hands out separate databases.
- The authorization matrix is proven by the two cases that **fail** on current code, which is the evidence
  for REQ-004 and REQ-006.

## 12. Traceability: acceptance criteria to components

| AC | Component | Task |
|---|---|---|
| AC-1 | `tests/support`, `conductor-storage/tests/migrations.rs`, `tests/health.rs` | TSK-020-01 |
| AC-2 | nextest profile, `cargo llvm-cov` invocation | TSK-020-05 |
| AC-3 | `vitest.config.ts`, `test:unit` script | TSK-020-03 |
| AC-4 | `playwright.config.ts`, `test:e2e` script | TSK-020-04 |
| AC-5 | `eslint.config.js`, `lint` script | TSK-020-03 |
| AC-6 | `.github/workflows/ci.yml` | TSK-020-05 |
| AC-7 | CI job running migrations against PostgreSQL | TSK-020-05 |
| AC-8 | `tests/authorization.rs` | TSK-020-02 |
| AC-9 | Branch protection plus a failing exit code | TSK-020-05 |
| AC-10 | `make test` | TSK-020-05 |
| AC-11 | Measured and recorded in TSK-020-01 and TSK-020-05 results | TSK-020-01, TSK-020-05 |

## 13. Task breakdown

| Task | Layer | Description | Depends on |
|---|---|---|---|
| [TSK-020-01](../task/01-REQ-020-automated-testing-ci/TSK-020-01-backend-test-harness.md) | BE | Dev-dependencies, the fixture, the migration test, the health smoke test | none |
| [TSK-020-02](../task/01-REQ-020-automated-testing-ci/TSK-020-02-authorization-suite.md) | BE | The authorization regression matrix across every endpoint and all three roles | TSK-020-01 |
| [TSK-020-03](../task/01-REQ-020-automated-testing-ci/TSK-020-03-frontend-unit-testing.md) | FE | vitest, Testing Library, eslint, and the first unit and component tests | none |
| [TSK-020-04](../task/01-REQ-020-automated-testing-ci/TSK-020-04-frontend-e2e.md) | FE | Playwright and one end-to-end flow | TSK-020-03 |
| [TSK-020-05](../task/01-REQ-020-automated-testing-ci/TSK-020-05-ci-pipeline.md) | Infra | CI workflow, PostgreSQL migration job, coverage, `make test` | 01, 02, 03, 04 |

TSK-020-01 and TSK-020-03 are independent and can run in parallel if two people are available.

## History

| Date | Change | Author |
|---|---|---|
| 2026-08-09 | Created | |
| 2026-08-10 | Recorded the merged backend harness while approval and remaining tasks are still open | Codex |
