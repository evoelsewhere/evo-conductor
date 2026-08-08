# Task

This directory stays empty until a design is approved.

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

None of this tooling is installed yet. See
[REQ-020](../requirement/REQ-020-automated-testing-ci.md).

## Register

| ID | Design | Layer | Title | Created | Status |
|---|---|---|---|---|---|
| None yet | | | | | |
