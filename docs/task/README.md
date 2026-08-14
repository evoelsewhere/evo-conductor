# Task

DES-007 tasks follow its approved design and now record the current as-built state. DES-020 and DES-011
started with lifecycle exceptions: their backend/registration implementations are present, while formal
design approval or reporting evidence remains incomplete. Task status records source reality and does not
retroactively approve those designs.

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
| [TSK-020-01](01-REQ-020-automated-testing-ci/TSK-020-01-backend-test-harness.md) | DES-020 | BE | Build the backend test harness | none | Implemented; reporting gaps |
| [TSK-020-02](01-REQ-020-automated-testing-ci/TSK-020-02-authorization-suite.md) | DES-020 | BE | Write the authorization regression suite | 020-01 | Partial |
| [TSK-020-03](01-REQ-020-automated-testing-ci/TSK-020-03-frontend-unit-testing.md) | DES-020 | FE | Set up frontend unit testing and linting | none | Todo |
| [TSK-020-04](01-REQ-020-automated-testing-ci/TSK-020-04-frontend-e2e.md) | DES-020 | FE | Set up Playwright and one end-to-end flow | 020-03 | Todo |
| [TSK-020-05](01-REQ-020-automated-testing-ci/TSK-020-05-ci-pipeline.md) | DES-020 | Infra | Build the CI pipeline | 01, 02, 03, 04 | Todo |
| [TSK-011-01](12-REQ-011-client-registration/TSK-011-01-installation-storage.md) | DES-011 | BE | Add installation registration storage | REQ-001, 006, 015 accepted | Implemented; PostgreSQL proof open |
| [TSK-011-02](12-REQ-011-client-registration/TSK-011-02-client-registration-api.md) | DES-011 | BE | Expose the client registration API | 011-01 | Implemented |
| [TSK-011-03](12-REQ-011-client-registration/TSK-011-03-evoflux-connection-service.md) | DES-011 | EvoFlux | Implement EvoFlux connection service | 011-02 | Implemented; packaged smoke open |
| [TSK-011-04](12-REQ-011-client-registration/TSK-011-04-evoflux-connection-ui.md) | DES-011 | EvoFlux FE | Build the EvoFlux connection experience | 011-03 | Implemented; Playwright open |
| [TSK-011-05](12-REQ-011-client-registration/TSK-011-05-console-installations.md) | DES-011 | FE | Show installations in the Conductor console | 011-02 | Implemented; UI tests open |
| [TSK-007-01](09-REQ-007-governed-resource-delivery/TSK-007-01-project-resource-schema.md) | DES-007 | BE | Add project-scoped resource schema and domain | none | Implemented; foundation gaps |
| [TSK-007-02](09-REQ-007-governed-resource-delivery/TSK-007-02-draft-import-validation.md) | DES-007 | BE | Build safe Draft import and validation | 007-01 | Implemented; shared fixtures open |
| [TSK-007-03](09-REQ-007-governed-resource-delivery/TSK-007-03-plugin-artifact-store.md) | DES-007 | BE | Add immutable Plugin artifact storage | 007-01 | Implemented; streaming proof open |
| [TSK-007-04](09-REQ-007-governed-resource-delivery/TSK-007-04-release-versioning.md) | DES-007 | BE | Implement transactional release versioning | 007-01–03 | Implemented; audit/PostgreSQL gaps |
| [TSK-007-05](09-REQ-007-governed-resource-delivery/TSK-007-05-effective-audience.md) | DES-007 | BE | Resolve access and Beta audience | 007-04 | Partial |
| [TSK-007-06](09-REQ-007-governed-resource-delivery/TSK-007-06-change-feed.md) | DES-007 | BE | Expose cursor changes and artifacts | 007-04–05 | Implemented on Conductor |
| [TSK-007-07](09-REQ-007-governed-resource-delivery/TSK-007-07-resource-studio-ui.md) | DES-007 | FE | Build Resource Studio and release UI | 007-02–05 | Implemented; UI tests open |
| [TSK-007-08](09-REQ-007-governed-resource-delivery/TSK-007-08-evoflux-managed-state.md) | DES-007 | EvoFlux | Persist managed state and reconcile Agent/Skill | 007-06 | Implemented with cursor; smart-fetch open |
| [TSK-007-09](09-REQ-007-governed-resource-delivery/TSK-007-09-evoflux-plugin-trust.md) | DES-007 | EvoFlux | Integrate Plugin staging and trust | 007-03, 06, 08 | Implemented; packaged E2E open |
| [TSK-007-10](09-REQ-007-governed-resource-delivery/TSK-007-10-evoflux-sync-ui.md) | DES-007 | EvoFlux FE | Build sync, diff and trust UI | 007-08–09 | Implemented; Playwright open |
| [TSK-007-11](09-REQ-007-governed-resource-delivery/TSK-007-11-inventory-ingestion.md) | DES-007 | BE | Ingest desired-versus-observed inventory | 007-06, 08–09 | Core implemented; fleet views partial |
| [TSK-007-12](09-REQ-007-governed-resource-delivery/TSK-007-12-cross-repo-proof.md) | DES-007 | Infra/QA | Prove cross-repo security and convergence | 007-01–11 | Partial |

Continue the exhaustive route matrix in **TSK-020-02**, then frontend test tooling, Playwright and CI in
TSK-020-03 through TSK-020-05. Governed delivery residual work is recorded per TSK-007 task.
