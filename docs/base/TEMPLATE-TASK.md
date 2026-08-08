# TEMPLATE — Task

Copy this file to `task/TSK-NNN-MM-<slug>.md`. Only create it once `DES-NNN` is `Approved`.
One task covers one layer. Do not combine frontend and backend work in a single task.

---

# TSK-NNN-MM — \<Title starting with a verb\>

| | |
|---|---|
| ID | TSK-NNN-MM |
| Created | YYYY-MM-DD |
| Updated | YYYY-MM-DD |
| Status | Todo |
| Layer | BE / FE / EvoFlux / Infra |
| Requirement | [REQ-NNN](../requirement/REQ-NNN-\<slug\>.md) |
| Design | [DES-NNN section X](../design/DES-NNN-\<slug\>.md) |
| Covers | AC-1, AC-3 |
| Depends on | TSK-NNN-MM, or none |
| Estimate | 0.5d / 1d / 2d |
| Branch | `feat/REQ-NNN-<slug>` |

## 1. Goal

<!-- One paragraph. What works once this task is complete. -->

## 2. Files in scope

| File | Action |
|---|---|
| `crates/...` | create / modify |

## 3. Implementation steps

1.
2.

## 4. Required tests

<!-- Keep the table for this task's layer and delete the others. -->

### Layer BE (Rust)

| Type | Tool | Must cover |
|---|---|---|
| Unit, pure domain | `cargo test` in `conductor-domain` | Business rules, parsing and serialization, boundary values |
| Repository | `cargo test` with sqlx `sqlite::memory:` | CRUD, unique constraints, migrations applying cleanly to an empty database |
| HTTP route | `axum::Router` with `tower::ServiceExt::oneshot` | Status codes, response shape, every authorization branch |
| Authorization regression | as above | For each of `admin`, `contribute` and `user`, assert `200` or `403` explicitly |

Mandatory: every new endpoint is tested against all three primary roles, including cases where the
expected outcome is that all three are allowed.

### Layer FE (React)

| Type | Tool | Must cover |
|---|---|---|
| Unit | `vitest` | Pure functions, formatters, API client with `fetch` mocked |
| Component | `vitest` with `@testing-library/react` | Render for each state: loading, empty, error, populated |
| Role-based rendering | `vitest` with `@testing-library/react` | Navigation entries and action buttons are absent for insufficient roles |
| End to end | `playwright` | Primary flow from sign-in to result, with screenshots |

Mandatory: the empty state and the error state always have tests. These are the states that break most
often and are noticed last.

### Layer EvoFlux (Python and React)

| Type | Tool | Must cover |
|---|---|---|
| Unit and integration | `pytest` | Synchronization logic, file writes, conflict detection |
| Static | `ruff check`, `ruff format --check`, `ty check` | Every file touched |
| Frontend | `vitest`, `playwright` | Only if the change is visible in the UI |

## 5. Commands and reports

```bash
# Backend
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo nextest run --profile ci
cargo llvm-cov --lcov --output-path target/coverage/lcov.info
```

```bash
# Frontend
cd apps/web
bun run typecheck
bun run lint
bun run test:unit -- --reporter=verbose
bun run test:e2e -- --reporter=html
```

```bash
# EvoFlux
uv run ruff check app/ tests/ && uv run ruff format --check app/ tests/
uv run ty check app/
uv run pytest --no-cov -q
```

Report locations: `target/nextest/ci/junit.xml`, `target/coverage/`,
`apps/web/playwright-report/`, plus screenshots embedded in section 7 below.

## 6. Definition of done

- [ ] Implementation complete and running locally
- [ ] Every acceptance criterion listed in the metadata is covered by an automated test, recorded in section 7
- [ ] Every command in section 5 runs clean, with no warning suppressed or ignored
- [ ] Real build succeeds: backend `cargo build --release`, frontend `bun run build`
- [ ] Affected documentation updated: `README`, `architecture.md`, the relevant register
- [ ] Section 7 contains real output, not a paraphrase
- [ ] No new clippy warning and no new type error introduced

## 7. Results

<!-- Fill in on completion. Paste real output; do not summarize it in prose.
     A task with an empty results section cannot be closed. -->

### Traceability: acceptance criteria to tests

| AC | Test case | File | Result |
|---|---|---|---|
| AC-1 | | | Pass / Fail |

### Command output

```
<paste the unmodified output of the commands in section 5>
```

### Screenshots

<!-- Required for any frontend task with a visible change.
     For a bug fix, include before and after using identical reproduction steps. -->

### Notes

<!-- Deviations from the design, technical debt accepted, follow-up work that needs its own task. -->

## History

| Date | Status | Note |
|---|---|---|
| YYYY-MM-DD | Todo | Created |
