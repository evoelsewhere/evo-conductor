# REQ-020 — Automated testing and CI

| | |
|---|---|
| ID | REQ-020 |
| Created | 2026-08-09 |
| Updated | 2026-08-14 |
| Status | Accepted (2026-08-09) — partial implementation |
| Priority | P0 |
| Build order | Step 1 of 23 |
| Spec section | [requirements.md section 16](../requirements.md), addition |
| Source | Code review 2026-08-09 |
| Depends on | none |
| Blocks | the definition of done of every task |
| Repositories | `evo-conductor` |
| Design | [DES-020](../design/01-DES-020-automated-testing-ci.md) |

## 1. Context

At creation, the Rust workspace contained zero tests and `apps/web/package.json` declared no test tooling
or lint script. The backend harness has since landed; frontend test infrastructure and CI remain absent.

Meanwhile the system holds passwords, JWT signing material and connection tokens for an entire team, and
is about to distribute configuration onto member machines. Section 16 of the specification lists sixteen
acceptance criteria; verified only by hand, they will not stay verified.

Every task template in [base/TEMPLATE-TASK.md](../base/TEMPLATE-TASK.md) requires tests to run. This
requirement builds the infrastructure that makes that requirement meaningful.

## 2. Requirement

The project shall have automated test infrastructure for backend and frontend, continuous integration
that runs it on every change, and a regression suite covering authentication and authorization.

## 3. Implementation status

| Implemented | Missing |
|---|---|
| Backend unit, repository, SQLite pool-isolation/migration and Axum route harness from merged PR #1 | `cargo nextest` JUnit and `cargo llvm-cov` reports |
| Shared `TestApp`, user seeding, JWT helpers and isolated database URLs | PostgreSQL migration execution in CI |
| Domain, storage and Axum integration suites; the 2026-08-14 workspace run passes 94 tests | Complete endpoint/three-role authorization matrix |
| `cargo fmt`, strict clippy, TypeScript typecheck and production builds are runnable | Conductor Vitest/Testing Library and committed Playwright scripts/reporting |
| Ad-hoc Playwright visual evidence exists for the member analytics PR | GitHub Actions or another merge-blocking CI workflow |
| | One full-suite local command and measured/targeted suite duration |

### Acceptance progress

| AC | State |
|---|---|
| AC-1 | Implemented |
| AC-8 | Partial — protected feature routes have negative tests, not every endpoint/role combination |
| AC-11 | Partial — the current suite is fast enough for local use, but no formal target or CI measurement is recorded |
| AC-2–AC-7, AC-9, AC-10 | Not complete |

## 4. Acceptance criteria

| ID | Criterion |
|---|---|
| AC-1 | Backend supports unit tests, repository tests against `sqlite::memory:`, and HTTP route tests using `axum` with `tower::ServiceExt::oneshot` |
| AC-2 | `cargo nextest` produces a JUnit XML report; `cargo llvm-cov` produces coverage output |
| AC-3 | Frontend has `vitest` and `@testing-library/react` with a `test:unit` script |
| AC-4 | Frontend has `playwright` with a `test:e2e` script producing an HTML report and screenshots on failure |
| AC-5 | Frontend has `eslint` with a `lint` script |
| AC-6 | CI runs on every push and pull request: `cargo fmt --check`, `cargo clippy -- -D warnings`, backend tests, frontend typecheck, lint and unit tests |
| AC-7 | CI applies the migration sequence against PostgreSQL and SQLite, satisfying [REQ-001](03-REQ-001-versioned-migrations.md) AC-5 |
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
- Should the EvoFlux-side work in [REQ-012](13-REQ-012-resource-sync-client.md) be tested in that
  repository's existing pipeline rather than here? Recommended: yes, each repository runs its own suite.

## History

| Date | Change | Author |
|---|---|---|
| 2026-08-09 | Created | |
| 2026-08-10 | Recorded merged backend harness and remaining frontend/reporting/CI gaps | Codex |
| 2026-08-14 | Reconciled the expanded 94-test backend suite and confirmed frontend test/lint/CI gaps remain | Codex |
