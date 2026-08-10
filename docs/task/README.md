# Task

The DES-020 tasks follow the normal lifecycle. DES-011 began as pre-approval planning, but implementation
proceeded later by explicit user direction. REQ-011 is now accepted; DES-011 approval remains unrecorded.
Its tasks stay `In Review` with results and remaining gaps recorded under that design lifecycle exception.

## When to create a task

A `TSK-NNN-MM` is created only after `DES-NNN` has moved to `Approved`. The task list comes from section
13 of the design; do not invent tasks outside it.

## How to create one

1. Copy [../base/TEMPLATE-TASK.md](../base/TEMPLATE-TASK.md) to `TSK-NNN-MM-<slug>.md`.
2. One task covers one layer. Do not combine frontend and backend work in a single task.
3. State which acceptance criteria the task covers. Every criterion in the requirement must be claimed by
   at least one task.
4. Keep the test table for the task's layer and delete the others.
5. Update the register below and in [../README.md](../README.md).

## Closing a task

A task cannot be closed while section 7, results, is empty. That section must contain the real output of
the test commands, not a description of them. Any frontend task with a visible change must include
screenshots; a bug fix must include before and after captured with identical reproduction steps.

## Test tooling by layer

| Layer | Unit | Integration | End to end | Static |
|---|---|---|---|---|
| Backend, Rust | `cargo test` | sqlx `sqlite::memory:`, `axum` with `tower::oneshot` | not applicable | `cargo fmt --check`, `cargo clippy -- -D warnings` |
| Frontend, React | `vitest` | `vitest` with `@testing-library/react` | `playwright` | `tsc -b --noEmit`, `eslint` |
| EvoFlux | `pytest` | `pytest` | `playwright` | `ruff`, `ty` |

Reports: `cargo nextest` produces JUnit XML, `cargo llvm-cov` produces coverage, `playwright` produces an
HTML report with screenshots. Commands are listed in
[../base/TEMPLATE-TASK.md section 5](../base/TEMPLATE-TASK.md).

The backend harness and static Rust tooling are available. Frontend unit/e2e integration, reports and CI
remain tracked by [REQ-020](../requirement/01-REQ-020-automated-testing-ci.md).

## Register

| ID | Design | Layer | Title | Depends on | Status |
|---|---|---|---|---|---|
| [TSK-020-01](01-REQ-020-automated-testing-ci/TSK-020-01-backend-test-harness.md) | DES-020 | BE | Build the backend test harness | none | In Review |
| [TSK-020-02](01-REQ-020-automated-testing-ci/TSK-020-02-authorization-suite.md) | DES-020 | BE | Write the authorization regression suite | 020-01 | Todo |
| [TSK-020-03](01-REQ-020-automated-testing-ci/TSK-020-03-frontend-unit-testing.md) | DES-020 | FE | Set up frontend unit testing and linting | none | Todo |
| [TSK-020-04](01-REQ-020-automated-testing-ci/TSK-020-04-frontend-e2e.md) | DES-020 | FE | Set up Playwright and one end-to-end flow | 020-03 | Todo |
| [TSK-020-05](01-REQ-020-automated-testing-ci/TSK-020-05-ci-pipeline.md) | DES-020 | Infra | Build the CI pipeline | 01, 02, 03, 04 | Todo |
| [TSK-011-01](12-REQ-011-client-registration/TSK-011-01-installation-storage.md) | DES-011 | BE | Add installation registration storage | REQ-001, 006, 015 accepted | In Review |
| [TSK-011-02](12-REQ-011-client-registration/TSK-011-02-client-registration-api.md) | DES-011 | BE | Expose the client registration API | 011-01 | In Review |
| [TSK-011-03](12-REQ-011-client-registration/TSK-011-03-evoflux-connection-service.md) | DES-011 | EvoFlux | Implement EvoFlux connection service | 011-02 | In Review |
| [TSK-011-04](12-REQ-011-client-registration/TSK-011-04-evoflux-connection-ui.md) | DES-011 | EvoFlux FE | Build the EvoFlux connection experience | 011-03 | In Review |
| [TSK-011-05](12-REQ-011-client-registration/TSK-011-05-console-installations.md) | DES-011 | FE | Show installations in the Conductor console | 011-02 | In Review |

Finish the reporting/measurement gaps in **TSK-020-01**, then continue TSK-020-02 through TSK-020-05.
