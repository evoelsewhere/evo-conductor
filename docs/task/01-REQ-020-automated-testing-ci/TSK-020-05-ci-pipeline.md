# TSK-020-05 — Build the CI pipeline

| | |
|---|---|
| ID | TSK-020-05 |
| Created | 2026-08-09 |
| Updated | 2026-08-09 |
| Status | Todo |
| Layer | Infra |
| Requirement | [REQ-020](../../requirement/01-REQ-020-automated-testing-ci.md) |
| Design | [DES-020 sections 10, 12](../../design/01-DES-020-automated-testing-ci.md) |
| Covers | AC-2, AC-6, AC-7, AC-9, AC-10, AC-11 |
| Depends on | TSK-020-01, TSK-020-02, TSK-020-03, TSK-020-04 |
| Estimate | 1d |
| Branch | `feat/REQ-020-ci` |

## 1. Goal

Every push and pull request runs the full suite automatically, a failure blocks merge, and the same suite
runs locally with one command.

This also delivers AC-7, the first time this codebase has ever run against PostgreSQL.

## 2. Files in scope

| File | Action |
|---|---|
| `.github/workflows/ci.yml` | create |
| `.config/nextest.toml` | create, the `ci` profile with JUnit output |
| `Makefile` | modify, add a `test` target |

The repository currently has **no workflow directory at all**, so the CI platform is an assumption to
confirm before starting — see section 7.

## 3. Implementation steps

1. `.config/nextest.toml`: a `ci` profile emitting JUnit XML to `target/nextest/ci/junit.xml`.
2. `.github/workflows/ci.yml` with three jobs:

   **backend** — `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
   `cargo nextest run --profile ci`, then `cargo llvm-cov --lcov`. Upload the JUnit XML and the coverage
   file as artifacts.

   **backend-postgres** — a PostgreSQL service container; run only the migration test with
   `CONDUCTOR_DATABASE_URL` pointing at it. This satisfies AC-7 and
   [REQ-001](../../requirement/03-REQ-001-versioned-migrations.md) AC-5.

   **frontend** — `bun install --frozen-lockfile`, `bun run typecheck`, `bun run lint`,
   `bun run test:unit`, `bun run build`. Playwright runs here too; upload the HTML report on failure.

3. Add a **non-blocking** step running `cargo nextest run --run-ignored all`, reporting the known expected
   failures from TSK-020-02 without failing the build. This keeps them visible instead of forgotten.
4. Cache the cargo registry, the target directory, and the bun store.
5. `make test`: run the backend and frontend suites in sequence with one command (AC-10).
6. Record the CI wall-clock time (AC-11) and state a target.
7. Enable branch protection requiring these jobs, or record who must enable it if that needs repository
   admin rights the implementer does not have.

## 4. Required tests

This task delivers test infrastructure rather than tests. It is verified by observation:

| Check | How |
|---|---|
| CI runs on push and pull request | Push the branch, link the run |
| A failing test turns CI red | Temporarily break one assertion, confirm red, revert |
| Migrations apply on PostgreSQL | The `backend-postgres` job passes |
| JUnit XML and coverage produced | Artifacts present on the run |
| `make test` works locally | Run it from a clean checkout |

## 5. Commands and reports

```bash
cargo nextest run --profile ci
cargo llvm-cov --lcov --output-path target/coverage/lcov.info
make test
```

Reports: `target/nextest/ci/junit.xml`, `target/coverage/lcov.info`,
`apps/web/playwright-report/`.

## 6. Definition of done

- [ ] CI runs on every push and pull request
- [ ] All three jobs pass on the branch
- [ ] The PostgreSQL migration job passes, or its failure is recorded as a finding for REQ-001
- [ ] A deliberately broken test was shown to turn CI red, and the break reverted
- [ ] The `--run-ignored all` step reports the known failures without blocking
- [ ] JUnit XML and coverage uploaded as artifacts
- [ ] `make test` runs everything locally
- [ ] Branch protection enabled, or the blocker named in section 7
- [ ] Section 7 contains real output and a link to a green run

## 7. Results

### Platform confirmation

<!-- State the CI platform actually used. The repository had no workflow directory when this task was
     written, so the assumption of GitHub Actions was unverified. -->

### Traceability: acceptance criteria to checks

| AC | Check | Result |
|---|---|---|
| AC-2 | JUnit XML and coverage artifacts present | |
| AC-6 | All jobs run on push and pull request | |
| AC-7 | Migrations apply on PostgreSQL | |
| AC-9 | Broken test turns CI red and blocks merge | |
| AC-10 | `make test` runs the full suite | |
| AC-11 | CI wall-clock time, with a stated target | |

### PostgreSQL outcome

<!-- This is the first PostgreSQL run in the project's history. If placeholder syntax, dialect
     differences or type mapping break, record the exact error here — it is a finding for
     REQ-001 and possibly a defect in existing queries, not a CI problem to work around. -->

### Command output

```
<paste the unmodified output, and link the CI run>
```

### Notes

<!-- Caching decisions, runtime, anything deferred. -->

## History

| Date | Status | Note |
|---|---|---|
| 2026-08-09 | Todo | Created |
