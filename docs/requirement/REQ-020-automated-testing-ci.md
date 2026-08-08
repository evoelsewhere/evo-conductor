# REQ-020 — Automated testing and CI

| | |
|---|---|
| ID | REQ-020 |
| Created | 2026-08-09 |
| Updated | 2026-08-09 |
| Status | Draft |
| Priority | P0 |
| Spec section | [requirements.md section 16](../requirements.md), addition |
| Source | Code review 2026-08-09 |
| Depends on | none |
| Blocks | the definition of done of every task |
| Repositories | `evo-conductor` |
| Design | Not created; requires acceptance |

## 1. Context

The Rust workspace contains zero tests: searching `crates/` for `#[test]` and `#[tokio::test]` returns no
matches. `apps/web/package.json` declares no test tooling and no lint script.

Meanwhile the system holds passwords, JWT signing material and connection tokens for an entire team, and
is about to distribute configuration onto member machines. Section 16 of the specification lists twelve
acceptance criteria; verified only by hand, they will not stay verified.

Every task template in [base/TEMPLATE-TASK.md](../base/TEMPLATE-TASK.md) requires tests to run. This
requirement builds the infrastructure that makes that requirement meaningful.

## 2. Requirement

The project shall have automated test infrastructure for backend and frontend, continuous integration
that runs it on every change, and a regression suite covering authentication and authorization.

## 3. Implementation status

| Implemented | Missing |
|---|---|
| `cargo` provides the test harness | Any test in `crates/` |
| `tsc -b --noEmit` is available through the `typecheck` script | `vitest`, Testing Library, Playwright, ESLint |
| | A CI workflow; this repository has no workflow directory |
| | A single-command local test entry point |

## 4. Acceptance criteria

| ID | Criterion |
|---|---|
| AC-1 | Backend supports unit tests, repository tests against `sqlite::memory:`, and HTTP route tests using `axum` with `tower::ServiceExt::oneshot` |
| AC-2 | `cargo nextest` produces a JUnit XML report; `cargo llvm-cov` produces coverage output |
| AC-3 | Frontend has `vitest` and `@testing-library/react` with a `test:unit` script |
| AC-4 | Frontend has `playwright` with a `test:e2e` script producing an HTML report and screenshots on failure |
| AC-5 | Frontend has `eslint` with a `lint` script |
| AC-6 | CI runs on every push and pull request: `cargo fmt --check`, `cargo clippy -- -D warnings`, backend tests, frontend typecheck, lint and unit tests |
| AC-7 | CI applies the migration sequence against PostgreSQL and SQLite, satisfying [REQ-001](REQ-001-versioned-migrations.md) AC-5 |
| AC-8 | An authorization regression suite covers every endpoint against all three primary roles |
| AC-9 | A failing test makes CI red and blocks merge |
| AC-10 | `make test` runs the full suite locally in one command |
| AC-11 | The suite completes fast enough to run on every change; a target is stated and measured |

## 5. Out of scope

- A specific coverage threshold. Measure first, set a target afterwards.
- Load and performance testing. Reconsider at P2.
- Deployment automation.

## 6. Risks

| # | Risk | Severity | Mitigation |
|---|---|---|---|
| 1 | Treated as work that produces no visible feature and deferred indefinitely | High | It is P0 and gates the definition of done; sequence it with the foundation work |
| 2 | Playwright makes CI slow | Medium | Limit end-to-end coverage to primary flows and let unit tests carry the rest |
| 3 | Tests are written to satisfy the process rather than to catch defects | Medium | AC-8 targets the area where real defects already exist |
| 4 | PostgreSQL in CI adds setup complexity | Low | A service container is sufficient |

## 7. Open questions

- Is CI hosted on GitHub Actions? The repository currently has no workflow directory, so the platform is
  unconfirmed.
- Should the EvoFlux-side work in [REQ-012](REQ-012-resource-sync-client.md) be tested in that
  repository's existing pipeline rather than here? Recommended: yes, each repository runs its own suite.

## History

| Date | Change | Author |
|---|---|---|
| 2026-08-09 | Created | |
